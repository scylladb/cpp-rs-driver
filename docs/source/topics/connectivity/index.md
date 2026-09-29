# Cluster Connectivity

The driver starts with one or more contact points, discovers the cluster
topology, and opens connections to the nodes it needs. Choose the connectivity
model based on whether the application can reach the addresses advertised by
the cluster nodes:

* [Direct, VPC peering, and TGW](direct-vpc-peering-and-tgw.md) use ordinary
  contact points because every discovered node address is reachable.
* [Client Routes](client-routes.md) map otherwise unreachable node addresses to
  per-node proxy endpoints for AWS PrivateLink, Google Cloud Private Service
  Connect, and similar private endpoint services.

Cloud network provisioning is separate from driver configuration. Follow the
linked ScyllaDB Cloud guides for the selected model, then configure the driver
as described here.

```{eval-rst}
.. toctree::
  :hidden:

  direct-vpc-peering-and-tgw
  client-routes
```
