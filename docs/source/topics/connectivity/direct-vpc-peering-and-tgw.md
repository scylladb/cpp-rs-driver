# Direct, VPC Peering, and TGW Connectivity

Use this model when the application can open a connection to every node address
advertised by the cluster. Direct connections, VPC-peered networks, and AWS
Transit Gateway (TGW) attachments differ at the network layer but use the same
driver configuration.

## Driver configuration

Configure reachable node addresses as ordinary contact points. Contact points
bootstrap topology discovery; after discovery, every node that the driver may
use must be reachable at its advertised address.

```c
CassCluster* cluster = cass_cluster_new();

cass_cluster_set_contact_points(cluster,
                                "10.0.1.10,10.0.2.10");

/* Configure other regular connection settings, then connect a session. */

cass_cluster_free(cluster);
```

Do **not** call `cass_cluster_add_client_routes_proxy()` for this connectivity
model. Configure [TLS](../security/tls.md) and
[authentication](../security/authentication.md) in the usual way when the
cluster requires them.

See [Getting Started](../getting-started.md) for the complete session setup and
query execution flow.

For network setup, see the ScyllaDB Cloud [connectivity options].

If the application cannot reach the advertised node addresses and the cluster
publishes per-node proxy mappings, use [Client Routes](client-routes.md)
instead.

[connectivity options]: https://cloud.docs.scylladb.com/stable/cluster-connections/connectivity-options.html
