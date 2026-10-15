# Client Routes (Private Networking)

When connecting to ScyllaDB Cloud through private networking such as AWS
PrivateLink or Google Cloud Private Service Connect, nodes are not reachable at
the addresses they advertise to each other. Instead, each node is reachable
through a proxy endpoint stored in `system.client_routes`. The Client Routes
feature tells the driver to use these proxy endpoints.

## Overview

In a Client Routes connection:

1. Each proxy endpoint is identified by a non-empty **connection ID** assigned
   to the private connection by the cloud infrastructure.
2. Ordinary contact points remain required as bootstrap endpoints. On session
   startup and every metadata refresh, the driver queries
   `system.client_routes` and filters its rows by the configured connection
   IDs. The resulting hostname or IP address and port replace each node's
   otherwise unreachable advertised address.
3. The driver subscribes to `CLIENT_ROUTES_CHANGE` events and updates its
   routing when the server reports a change, without waiting for a full
   metadata refresh.

## Limitations

* Mixed direct and routed topologies are not supported. Every discovered node
  must be reachable through a configured Client Routes proxy and have a
  matching entry in `system.client_routes` for at least one configured
  connection ID.
* TLS is not supported in Client Routes mode. If TLS is configured with
  `cass_cluster_set_ssl()`, the future returned by `cass_session_connect()`
  completes with an error.
* Advanced shard awareness, in which the driver selects a source port to
  target a shard, is disabled in Client Routes mode because the proxy does not
  preserve it. Basic shard awareness remains available, so the driver can
  still route requests to the appropriate shard.

## Prerequisites

Before configuring the driver, complete the private endpoint setup described in
[Client Routes connectivity](../connectivity/client-routes.md). Obtain the
following values from that setup:

* One or more bootstrap contact points that the client can reach.
* Every connection ID that the client may use.

The client must also be able to resolve and reach the proxy endpoints returned
for those connection IDs. If it cannot use the hostnames stored in
`system.client_routes`, configure an override as described below.

## Basic usage

Configure the bootstrap contact points first, then add a Client Routes proxy.
The first call to `cass_cluster_add_client_routes_proxy()` enables Client
Routes mode:

```c
CassCluster* cluster = cass_cluster_new();

/* A bootstrap endpoint supplied by the cloud setup. */
cass_cluster_set_contact_points(cluster, "bootstrap.example.com");

/* NULL selects the hostname from system.client_routes. */
cass_cluster_add_client_routes_proxy(cluster, "connection-id", NULL);

/* ... connect a session ... */

cass_cluster_free(cluster);
```

The contact points establish the initial connection and let the driver read
`system.client_routes`. They do not replace the proxy configuration, and proxy
hostnames do not replace the bootstrap contact points.

## Multiple connection IDs

If the deployment routes traffic through more than one proxy, such as one per
availability zone, call `cass_cluster_add_client_routes_proxy()` once for each
connection ID. Calls append proxy configurations:

```c
cass_cluster_add_client_routes_proxy(cluster, "connection-id-a", NULL);
cass_cluster_add_client_routes_proxy(cluster, "connection-id-b", NULL);
```

The driver considers table entries matching any configured connection ID when
it discovers routes.

## Overriding the hostname

By default, the driver uses the proxy hostname stored in
`system.client_routes`. Pass a non-empty hostname to use a different DNS name:

```c
cass_cluster_add_client_routes_proxy(cluster,
                                     "connection-id",
                                     "proxy.example.com");
```

An IP address can also be used as the override:

```c
cass_cluster_add_client_routes_proxy(cluster,
                                     "connection-id",
                                     "192.0.2.10");
```

Passing `NULL` or an empty string as the override selects the hostname from the
table.

Use `cass_cluster_add_client_routes_proxy_n()` when the string lengths are
known explicitly or the strings are not null-terminated:

```c
const char connection_id[] = "connection-id";
const char hostname_override[] = "proxy.example.com";

cass_cluster_add_client_routes_proxy_n(
    cluster,
    connection_id,
    sizeof(connection_id) - 1,
    hostname_override,
    sizeof(hostname_override) - 1);
```

To select the hostname from `system.client_routes` with this variant, pass
`NULL` and zero for the hostname pointer and length.

## Differences from a regular connection

In a regular connection, the driver connects to discovered nodes at their
advertised addresses. In Client Routes mode, it replaces those addresses with
the proxy routes discovered in `system.client_routes` and keeps those routes
up to date.

Other supported cluster settings remain usable, including
[authentication](../security/authentication.md), compression,
[execution profiles](execution-profiles.md), and timeouts. The TLS and
shard-aware source-port exceptions, and the requirement not to mix routed and
direct nodes, are described in [Limitations](#limitations).
