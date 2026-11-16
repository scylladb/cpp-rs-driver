# Consistency

A setting that defines a successful write or read by the number of cluster
replicas that acknowledge the write or respond to the read request,
respectively.

## Default consistency

The default read and write consistency is `CASS_CONSISTENCY_LOCAL_QUORUM`.
The default serial consistency for Lightweight Transactions (LWTs) is
`CASS_CONSISTENCY_LOCAL_SERIAL`.

These defaults apply to simple statements, bound prepared statements, and batches.
Explicit cluster, execution profile, statement, or batch settings take precedence
at their respective scopes. An execution profile whose regular consistency is
unset uses `CASS_CONSISTENCY_LOCAL_QUORUM`, even when the cluster default has been
overridden. Its unset serial consistency inherits the cluster setting.

Applications that need the previous read and write default can set
`CASS_CONSISTENCY_LOCAL_ONE` explicitly. A local quorum requires responses from a
majority of replicas in the local datacenter, which can affect latency and
availability compared with `CASS_CONSISTENCY_LOCAL_ONE`.

## Consistency Levels

### Read and Write Consistency Levels

The consistency level determines the number of replicas on which the read/write
must respond/succeed before returning an acknowledgment to the client
application. Descriptions for each read/write consistency level can be found
[here](https://docs.scylladb.com/manual/stable/cql/consistency.html).

<table class="table table-striped table-hover table-condensed">
  <thead>
  <tr>
   <th>Level</th>
   <th>Driver</th>
  </tr>
  </thead>

  <tbody>
  <tr>
   <td>All</td>
   <td><code>CASS_CONSISTENCY_ALL</code></td>
  </tr>
  <tr>
   <td>Each Quorum</td>
   <td><code>CASS_CONSISTENCY_EACH_QUORUM</code></td>
  </tr>
  <tr>
   <td>Quorum</td>
   <td><code>CASS_CONSISTENCY_QUORUM</code></td>
  </tr>
  <tr>
   <td>Local Quorum</td>
   <td><code>CASS_CONSISTENCY_LOCAL_QUORUM</code></td>
  </tr>
  <tr>
   <td><b>One</b></td>
   <td><code><b>CASS_CONSISTENCY_ONE</b></code></td>
  </tr>
  <tr>
   <td>Two</td>
   <td><code>CASS_CONSISTENCY_TWO</code></td>
  </tr>
  <tr>
   <td>Three</td>
   <td><code>CASS_CONSISTENCY_THREE</code></td>
  </tr>
  <tr>
   <td>Local One</td>
   <td><code>CASS_CONSISTENCY_LOCAL_ONE</code></td>
  </tr>
  <tr>
   <td>Any</td>
   <td><code>CASS_CONSISTENCY_ANY</code></td>
  </tr>
  <tr>
   <td>Serial</td>
   <td><code>CASS_CONSISTENCY_SERIAL</code></td>
  </tr>
  <tr>
   <td>Local Serial</td>
   <td><code>CASS_CONSISTENCY_LOCAL_SERIAL</code></td>
  </tr>
  </tbody>
</table>

**NOTE:** Consistency level `CASS_CONSISTENCY_ANY` is only valid for write operation statements.

## Setting Consistency Level

A ['CassStatement'](https://cpp-rs-driver.docs.scylladb.com/stable/api/struct.CassFuture/) object
can have its consistency level altered at anytime before the statement is
executed by the session.

```c
CassStatement* statement = NULL;

/* Create a simple or prepared statment */

/* Ensure the session executed statement has strong consistency */
cass_statement_set_consistency(statement, CASS_CONSISTENCY_QUORUM);

cass_statement_free(statement);
```

**NOTE:** Consistency level is ignored for `USE`, `TRUNCATE`, `CREATE` and `ALTER`
statements, and some, like `CASS_CONSISTENCY_ANY`, aren’t allowed in all situations.
