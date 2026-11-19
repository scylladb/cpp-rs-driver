# Client Routes Connectivity

Use Client Routes when the cluster is exposed through AWS PrivateLink, Google
Cloud Private Service Connect, or a similar private endpoint service and the
application cannot reach the addresses advertised by the cluster nodes.

In this model, the private connection provides bootstrap contact points and one
or more connection IDs. For every cluster node, `system.client_routes` maps a
connection ID to a reachable proxy hostname or IP address and port. The driver
uses those mappings instead of connecting to advertised node addresses.

## Driver configuration

Configure a reachable bootstrap contact point and add the connection ID for the
private endpoint:

```c
CassCluster* cluster = cass_cluster_new();

cass_cluster_set_contact_points(cluster, "bootstrap.example.com");
cass_cluster_add_client_routes_proxy(cluster, "connection-id", NULL);

/* Configure other connection settings, then connect a session. */

cass_cluster_free(cluster);
```

See the [full Client Routes feature documentation]
for discovery behavior, multiple connection IDs, hostname overrides, and
limitations.

Provision the private endpoint outside the driver by following the ScyllaDB
Cloud [network access options] documentation.

[network access options]: https://cloud.docs.scylladb.com/stable/cluster-connections/connectivity-options.html
[full Client Routes feature documentation]: ../configuration/client-routes.md
