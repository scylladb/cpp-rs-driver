# Client Configuration

Client configuration allows an application to provide additional metadata to
the cluster which can be useful for troubleshooting and performing diagnostics.
In addition to the optional application metadata the cluster will automatically
be provided with the driver's name, driver's version, and a unique session
identifier.

## Application Options (Optional)

Application name and version metadata can be provided to the cluster during
configuration. This information can be used to isolate specific applications on
the server-side when troubleshooting or performing diagnostics on clusters that
support multiple applications.

```c
CassCluster* cluster = cass_cluster_new();

/* Assign a name for the application connecting to the cluster */
cass_cluster_set_application_name(cluster, "Application Name");

/* Assign a version for the application connecting to the cluster */
cass_cluster_set_application_version(cluster, "1.0.0");

/* ... */

cass_cluster_free(cluster);
```

## Client Identification

Each session is assigned a unique identifier (UUID) which can be used to
identify specific client connections server-side. The identifier can also be
retrieved client-side using the following function:

```c
CassSession* session = cass_session_new();

/* Retrieve the session's unique identifier */
CassUuid client_id = cass_session_get_client_id(session);

/* ... */

cass_session_free(session);
```

**Note**: A session's unique identifier is constant for its lifetime and does
          not change when re-establishing connection to a cluster.

## Driver Configuration Reporting

By default, each session reports its effective driver configuration to
ScyllaDB. The control connection sends one compact JSON document under the
`DRIVER_CONFIG` startup option. It uses the cross-driver schema version 1 and
describes connection, control-plane, and query settings such as timeouts,
socket options, and policies.

On supported ScyllaDB versions, the report is available in
`system.clients.client_options`. This table is node-local. Every connection of
the session also sends the same `SESSION_ID`, allowing the control connection's
report to be correlated with the session's other connections.

Applications that must not disclose configuration can disable only the report:

```c
CassCluster* cluster = cass_cluster_new();

cass_cluster_set_driver_config_reporting(cluster, cass_false);

/* ... */

cass_cluster_free(cluster);
```

Disabling configuration reporting does not disable `SESSION_ID`.
