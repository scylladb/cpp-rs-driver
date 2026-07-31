//! End-to-end TLS tests against a real, TLS-enabled ScyllaDB cluster started
//! via CCM. The driver is exercised exclusively through its C API, mirroring
//! how a C/C++ consumer would use it.
//!
//! These tests are ports of the ScyllaDB Rust Driver's `ccm::tls` integration
//! tests, adapted to drive this driver through the C API.

use std::ffi::CString;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;

use futures::StreamExt as _;
use rcgen::{
    BasicConstraints, CertificateParams, CertifiedIssuer, DistinguishedName, DnType, IsCa, KeyPair,
    SanType,
};
use tokio::fs::File;
use tokio::io::AsyncWriteExt as _;

use scylla_ccm_bridge::cluster::{Cluster, ClusterOptions};
use scylla_ccm_bridge::node::Node;
use scylla_ccm_bridge::{CLUSTER_VERSION, run_ccm_test_with_configuration};

use scylladb::api::cluster::{
    cass_cluster_free, cass_cluster_new, cass_cluster_set_contact_points, cass_cluster_set_ssl,
};
use scylladb::api::error::CassError;
use scylladb::api::future::{cass_future_error_code, cass_future_free, cass_future_wait};
use scylladb::api::session::{
    cass_session_close, cass_session_connect, cass_session_execute, cass_session_free,
    cass_session_new,
};
use scylladb::api::ssl::{
    cass_ssl_add_trusted_cert, cass_ssl_free, cass_ssl_new, cass_ssl_set_verify_flags,
};
use scylladb::api::statement::{cass_statement_free, cass_statement_new};
use scylladb::argconv::CassStrNulTerminated;

use crate::utils::{assert_cass_error_eq, setup_tracing};

use super::warn_if_partial_cluster_version;

// Verification flag values, as defined by the C API (`cassandra.h`).
// The Rust constants in the driver are crate-private, so we redefine the ABI
// values here, exactly as a C consumer would use them.
const CASS_SSL_VERIFY_NONE: i32 = 0x00;
const CASS_SSL_VERIFY_PEER_IDENTITY: i32 = 0x02;

fn cluster_3_nodes() -> ClusterOptions {
    ClusterOptions {
        name: "cluster_tls_3_node".to_string(),
        version: CLUSTER_VERSION.clone(),
        nodes_per_dc: vec![3],
        ..ClusterOptions::default()
    }
}

/// Generates and installs a per-node server certificate (signed by `ca`) and
/// enables `client_encryption_options` on the node.
///
/// `prepare_cert` customizes the certificate parameters (e.g. its SANs) based
/// on the node it is generated for.
async fn configure_node_tls(
    node: &mut Node,
    ca: &CertifiedIssuer<'static, KeyPair>,
    prepare_cert: impl FnOnce(CertificateParams, &Node) -> CertificateParams,
) {
    let params = prepare_cert(CertificateParams::new(vec![]).unwrap(), node);
    let key = KeyPair::generate().unwrap();
    let cert = params.signed_by(&key, ca).unwrap();

    let cert_file_path = node.node_dir().join("db.cert");
    let mut cert_file = File::create_new(&cert_file_path).await.unwrap();
    cert_file.write_all(cert.pem().as_bytes()).await.unwrap();

    let key_file_path = node.node_dir().join("db.key");
    let mut key_file = File::create_new(&key_file_path).await.unwrap();
    key_file
        .write_all(key.serialize_pem().as_bytes())
        .await
        .unwrap();

    let args = [
        ("client_encryption_options.enabled", "true"),
        (
            "client_encryption_options.certificate",
            cert_file_path.to_str().unwrap(),
        ),
        (
            "client_encryption_options.keyfile",
            key_file_path.to_str().unwrap(),
        ),
    ];
    node.updateconf(args).await.unwrap();
}

fn prepare_authority_cert_params() -> CertificateParams {
    let mut params =
        CertificateParams::new(vec!["cpp_rs_driver_integration_test_ca".to_owned()]).unwrap();
    params.distinguished_name = {
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CountryName, "PL");
        dn.push(DnType::OrganizationName, "scylladb");
        dn.push(DnType::OrganizationalUnitName, "cpp_rs_driver_ccm_tests");
        dn.push(DnType::CommonName, "cpp_rs_driver_ccm_tests");
        dn
    };
    params.use_authority_key_identifier_extension = true;
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);

    params
}

/// Runs a TLS integration test against a fresh 3-node cluster.
///
/// - `prepare_cert` customizes each node's server certificate.
/// - `cluster_config` applies extra cluster-wide configuration before the
///   nodes are configured (e.g. enabling client authentication).
/// - `test` is the actual test body; it receives the CA (so it can build a
///   matching client trust store) and the running cluster.
async fn run_ccm_tls_test(
    prepare_cert: impl Fn(CertificateParams, &Node) -> CertificateParams,
    cluster_config: impl AsyncFnOnce(Cluster) -> Cluster,
    test: impl AsyncFnOnce(&CertifiedIssuer<'static, KeyPair>, &mut Cluster),
) {
    warn_if_partial_cluster_version();

    let ca_key = KeyPair::generate().unwrap();
    let params = prepare_authority_cert_params();
    let ca = Arc::new(CertifiedIssuer::self_signed(params, ca_key).unwrap());

    run_ccm_test_with_configuration(
        cluster_3_nodes,
        {
            let ca = Arc::clone(&ca);
            |mut cluster: Cluster| async move {
                let ca_key_file_path = cluster.cluster_dir().join("ca.key");
                let mut ca_key_file = File::create_new(&ca_key_file_path).await.unwrap();
                ca_key_file
                    .write_all(ca.key().serialize_pem().as_bytes())
                    .await
                    .unwrap();

                let ca_cert_file_path = cluster.cluster_dir().join("ca.crt");
                let mut ca_cert_file = File::create_new(&ca_cert_file_path).await.unwrap();
                ca_cert_file.write_all(ca.pem().as_bytes()).await.unwrap();

                cluster
                    .updateconf([(
                        "client_encryption_options.truststore",
                        ca_cert_file_path.to_str().unwrap(),
                    )])
                    .await
                    .unwrap();

                let mut cluster = cluster_config(cluster).await;

                // The nodes must be configured one at a time. Every `ccm`
                // invocation loads the whole cluster, reading every node's
                // `node.conf` (`ClusterFactory.load` -> `Node.load`), while
                // `ccm <node> updateconf` rewrites its own `node.conf` in
                // place: `open(path, 'w')` truncates it and only then dumps
                // the new contents. Run concurrently, one process reads
                // another's momentarily empty file and ccm dies with
                // "TypeError: 'NoneType' object is not subscriptable".
                futures::stream::iter(cluster.nodes_mut().iter_mut())
                    .for_each(|node| configure_node_tls(node, &ca, &prepare_cert))
                    .await;

                cluster
            }
        },
        async |cluster: &mut Cluster| test(&ca, cluster).await,
    )
    .await
}

/// Establishes a TLS-enabled session to `contact_points` through the C API and,
/// if the connection succeeds, runs a trivial health-check query.
///
/// Returns the first non-OK [`CassError`] encountered (from connect or from the
/// health-check), or `CASS_OK` if everything succeeded.
///
/// # Safety
///
/// Must be called on a thread where blocking is acceptable (it blocks on the
/// driver futures via `cass_future_wait`).
unsafe fn connect_and_healthcheck(
    contact_points: &str,
    verify_flags: Option<i32>,
    trust_ca_pem: Option<&str>,
) -> CassError {
    unsafe {
        let mut cluster_raw = cass_cluster_new();

        let contact_points_c = CString::new(contact_points).unwrap();
        assert_cass_error_eq(
            CassError::CASS_OK,
            cass_cluster_set_contact_points(
                cluster_raw.borrow_mut(),
                CassStrNulTerminated::from_cstr(&contact_points_c),
            ),
        );

        let ssl_raw = cass_ssl_new();
        if let Some(ca_pem) = trust_ca_pem {
            let ca_pem_c = CString::new(ca_pem).unwrap();
            assert_cass_error_eq(
                CassError::CASS_OK,
                cass_ssl_add_trusted_cert(
                    ssl_raw.borrow(),
                    CassStrNulTerminated::from_cstr(&ca_pem_c),
                ),
            );
        }
        if let Some(flags) = verify_flags {
            cass_ssl_set_verify_flags(ssl_raw.borrow(), flags);
        }
        cass_cluster_set_ssl(cluster_raw.borrow_mut(), ssl_raw.borrow());

        let session_raw = cass_session_new();

        let connect_fut =
            cass_session_connect(session_raw.borrow(), cluster_raw.borrow().into_c_const());
        cass_future_wait(connect_fut.borrow());
        let connect_err = cass_future_error_code(connect_fut.borrow());
        cass_future_free(connect_fut);

        let result = if connect_err == CassError::CASS_OK {
            let query = c"SELECT host_id FROM system.local WHERE key='local'";
            let statement_raw = cass_statement_new(CassStrNulTerminated::from_cstr(query), 0);
            let exec_fut =
                cass_session_execute(session_raw.borrow(), statement_raw.borrow().into_c_const());
            cass_future_wait(exec_fut.borrow());
            let exec_err = cass_future_error_code(exec_fut.borrow());
            cass_future_free(exec_fut);
            cass_statement_free(statement_raw);

            let close_fut = cass_session_close(session_raw.borrow());
            cass_future_wait(close_fut.borrow());
            cass_future_free(close_fut);

            exec_err
        } else {
            connect_err
        };

        cass_session_free(session_raw);
        cass_ssl_free(ssl_raw);
        cass_cluster_free(cluster_raw);

        result
    }
}

/// Convenience wrapper around [`connect_and_healthcheck`] that derives the
/// contact points from the cluster and runs the blocking C-API work off the
/// async runtime.
async fn try_tls_connect(
    cluster: &Cluster,
    verify_flags: Option<i32>,
    trust_ca_pem: Option<String>,
) -> CassError {
    let contact_points = cluster
        .nodes()
        .iter()
        .map(|node| node.broadcast_rpc_address().to_string())
        .collect::<Vec<_>>()
        .join(",");

    tokio::task::spawn_blocking(move || unsafe {
        connect_and_healthcheck(&contact_points, verify_flags, trust_ca_pem.as_deref())
    })
    .await
    .unwrap()
}

/// Basic TLS test, with the server not requiring client authentication.
///
/// Checks that the driver can connect to a TLS-enabled cluster and execute
/// requests, both with full identity verification and with verification
/// disabled, and that connecting without a trusted CA fails.
#[tokio::test]
async fn connect_tls_no_client_auth() {
    setup_tracing();

    // Each node's certificate has a SAN matching its own broadcast RPC address.
    fn prepare_cert(mut params: CertificateParams, node: &Node) -> CertificateParams {
        params
            .subject_alt_names
            .push(SanType::IpAddress(node.broadcast_rpc_address()));
        params
    }

    async fn test(ca: &CertifiedIssuer<'static, KeyPair>, cluster: &mut Cluster) {
        let ca_pem = ca.pem();

        // PEER_IDENTITY: trusted CA, cert SAN matches the node IP -> connects.
        assert_cass_error_eq(
            CassError::CASS_OK,
            try_tls_connect(
                cluster,
                Some(CASS_SSL_VERIFY_PEER_IDENTITY),
                Some(ca_pem.clone()),
            )
            .await,
        );

        // NONE: verification disabled -> connects even without a trusted CA.
        assert_cass_error_eq(
            CassError::CASS_OK,
            try_tls_connect(cluster, Some(CASS_SSL_VERIFY_NONE), None).await,
        );

        // default: verification disabled -> connects even without a trusted CA.
        assert_cass_error_eq(
            CassError::CASS_OK,
            try_tls_connect(cluster, None, None).await,
        );

        // PEER_IDENTITY without a trusted CA -> certificate chain validation
        // fails, so the connection is rejected.
        let err = try_tls_connect(cluster, Some(CASS_SSL_VERIFY_PEER_IDENTITY), None).await;
        assert_ne!(
            err,
            CassError::CASS_OK,
            "expected connection to fail when the server CA is not trusted"
        );
    }

    run_ccm_tls_test(prepare_cert, async |c| c, test).await
}

/// Verifies that identity verification is actually enforced: if a node presents
/// a certificate whose subject alternative name does not match the node's IP
/// address, the driver must refuse to connect under `PEER_IDENTITY`, while
/// still connecting under `NONE` (which disables verification entirely).
///
/// Port of the Rust Driver's `test_tls_verifies_hostname`.
#[tokio::test]
async fn tls_verifies_hostname() {
    setup_tracing();

    // Every node gets a certificate with a SAN that does not match its IP.
    fn prepare_cert(mut params: CertificateParams, _node: &Node) -> CertificateParams {
        params
            .subject_alt_names
            .push(SanType::IpAddress(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
        params
    }

    async fn test(ca: &CertifiedIssuer<'static, KeyPair>, cluster: &mut Cluster) {
        let ca_pem = ca.pem();

        // PEER_IDENTITY: chain is valid but the SAN does not match the node IP
        // -> identity verification fails, so the connection is rejected.
        let err = try_tls_connect(
            cluster,
            Some(CASS_SSL_VERIFY_PEER_IDENTITY),
            Some(ca_pem.clone()),
        )
        .await;
        assert_ne!(
            err,
            CassError::CASS_OK,
            "expected connection to fail: certificate SAN does not match the node IP"
        );

        // NONE: verification disabled -> the SAN mismatch is ignored and the
        // connection succeeds.
        assert_cass_error_eq(
            CassError::CASS_OK,
            try_tls_connect(cluster, Some(CASS_SSL_VERIFY_NONE), Some(ca_pem)).await,
        );
    }

    run_ccm_tls_test(prepare_cert, async |c| c, test).await
}
