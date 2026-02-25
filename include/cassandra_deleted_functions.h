/*
 * This file contains declarations of functions that have been deleted
 * from the public API (`cassandra.h`) upon transition to the new C/C++ driver
 * implementation as a wrapper over the Rust Driver.
 *
 * Each function or function group is marked with a comment indicating
 * the reason for its deletion.
 *
 */

#ifndef __DELETED_H_INCLUDED__
#define __DELETED_H_INCLUDED__

#include "cassandra.h"

#ifdef __cplusplus
extern "C" {
#endif

/* Custom type - deleted due to no planned support for this legacy feature
 * in the Rust Driver. */

 /**
  * Appends a "custom" to the collection.
  *
  * @public @memberof CassCollection
  *
  * @param[in] collection
  * @param[in] class_name
  * @param[in] value The value is copied into the collection object; the
  * memory pointed to by this parameter can be freed after this call.
  * @param[in] value_size
  * @return CASS_OK if successful, otherwise an error occurred.
  */
 CASS_EXPORT CassError
 cass_collection_append_custom(CassCollection* collection,
                               const char* class_name,
                               const cass_byte_t* value,
                               size_t value_size);

/**
 * Same as cass_collection_append_custom(), but with lengths for string
 * parameters.
 *
 * @public @memberof CassCollection
 *
 * @param[in] collection
 * @param[in] class_name
 * @param[in] class_name_length
 * @param[in] value
 * @param[in] value_size
 * @return same as cass_collection_append_custom()
 *
 * @see cass_collection_append_custom()
 */
CASS_EXPORT CassError
cass_collection_append_custom_n(CassCollection* collection,
                                const char* class_name,
                                size_t class_name_length,
                                const cass_byte_t* value,
                                size_t value_size);

/**
 * Binds a "custom" to a query or bound statement at the specified index.
 *
 * @public @memberof CassStatement
 *
 * @param[in] statement
 * @param[in] index
 * @param[in] class_name
 * @param[in] value The value is copied into the statement object; the
 * memory pointed to by this parameter can be freed after this call.
 * @param[in] value_size
 * @return CASS_OK if successful, otherwise an error occurred.
 */
CASS_EXPORT CassError
cass_statement_bind_custom(CassStatement* statement,
                           size_t index,
                           const char* class_name,
                           const cass_byte_t* value,
                           size_t value_size);

/**
 * Same as cass_statement_bind_custom(), but with lengths for string
 * parameters.
 *
 * @public @memberof CassStatement
 *
 * @param[in] statement
 * @param[in] index
 * @param[in] class_name
 * @param[in] class_name_length
 * @param[in] value The value is copied into the statement object; the
 * memory pointed to by this parameter can be freed after this call.
 * @param[in] value_size
 * @return CASS_OK if successful, otherwise an error occurred.
 */
CASS_EXPORT CassError
cass_statement_bind_custom_n(CassStatement* statement,
                             size_t index,
                             const char* class_name,
                             size_t class_name_length,
                             const cass_byte_t* value,
                             size_t value_size);

/**
 * Binds a "custom" to all the values with the specified name.
 *
 * @public @memberof CassStatement
 *
 * @param[in] statement
 * @param[in] name
 * @param[in] class_name
 * @param[in] value The value is copied into the statement object; the
 * memory pointed to by this parameter can be freed after this call.
 * @param[in] value_size
 * @return CASS_OK if successful, otherwise an error occurred.
 */
CASS_EXPORT CassError
cass_statement_bind_custom_by_name(CassStatement* statement,
                                   const char* name,
                                   const char* class_name,
                                   const cass_byte_t* value,
                                   size_t value_size);

/**
 * Same as cass_statement_bind_custom_by_name(), but with lengths for string
 * parameters.
 *
 * @public @memberof CassStatement
 *
 * @param[in] statement
 * @param[in] name
 * @param[in] name_length
 * @param[in] class_name
 * @param[in] class_name_length
 * @param[in] value
 * @param[in] value_size
 * @return same as cass_statement_bind_custom_by_name()
 *
 * @see cass_statement_bind_custom_by_name()
 */
CASS_EXPORT CassError
cass_statement_bind_custom_by_name_n(CassStatement* statement,
                                     const char* name,
                                     size_t name_length,
                                     const char* class_name,
                                     size_t class_name_length,
                                     const cass_byte_t* value,
                                     size_t value_size);
/**
 * Sets a "custom" in a tuple at the specified index.
 *
 * @public @memberof CassTuple
 *
 * @param[in] tuple
 * @param[in] index
 * @param[in] class_name
 * @param[in] value The value is copied into the tuple object; the
 * memory pointed to by this parameter can be freed after this call.
 * @param[in] value_size
 * @return CASS_OK if successful, otherwise an error occurred.
 */
CASS_EXPORT CassError
cass_tuple_set_custom(CassTuple* tuple,
                      size_t index,
                      const char* class_name,
                      const cass_byte_t* value,
                      size_t value_size);

/**
 * Same as cass_tuple_set_custom(), but with lengths for string
 * parameters.
 *
 * @public @memberof CassTuple
 *
 * @param[in] tuple
 * @param[in] index
 * @param[in] class_name
 * @param[in] class_name_length
 * @param[in] value
 * @param[in] value_size
 * @return same as cass_tuple_set_custom()
 *
 * @see cass_tuple_set_custom()
 */
CASS_EXPORT CassError
cass_tuple_set_custom_n(CassTuple* tuple,
                        size_t index,
                        const char* class_name,
                        size_t class_name_length,
                        const cass_byte_t* value,
                        size_t value_size);


/* DataStax Enterprise-specific features
 * The driver does not support DataStax Enterprise. */

/**
 * Sets the secure connection bundle path for processing DBaaS credentials.
 *
 * This will pre-configure a cluster using the credentials format provided by
 * the DBaaS cloud provider.
 *
 * @param[in] cluster
 * @param[in] path Absolute path to DBaaS credentials file.
 * @return CASS_OK if successful, otherwise error occured.
 */
CASS_EXPORT CassError
cass_cluster_set_cloud_secure_connection_bundle(CassCluster* cluster,
                                                const char* path);

/**
 * Same as cass_cluster_set_cloud_secure_connection_bundle(), but with lengths
 * for string parameters.
 *
 * @see cass_cluster_set_cloud_secure_connection_bundle()
 *
 * @param[in] cluster
 * @param[in] path Absolute path to DBaaS credentials file.
 * @param[in] path_length Length of path variable.
 * @return CASS_OK if successful, otherwise error occured.
 */
CASS_EXPORT CassError
cass_cluster_set_cloud_secure_connection_bundle_n(CassCluster* cluster,
                                                  const char* path,
                                                  size_t path_length);

/**
 * Same as cass_cluster_set_cloud_secure_connection_bundle(), but it does not
 * initialize the underlying SSL library implementation. The SSL library still
 * needs to be initialized, but it's up to the client application to handle
 * initialization. This is similar to the function cass_ssl_new_no_lib_init(),
 * and its documentation should be used as a reference to properly initialize
 * the underlying SSL library.
 *
 * @see cass_ssl_new_no_lib_init()
 * @see cass_cluster_set_cloud_secure_connection_bundle()
 *
 * @param[in] cluster
 * @param[in] path Absolute path to DBaaS credentials file.
 * @return CASS_OK if successful, otherwise error occured.
 */
CASS_EXPORT CassError
cass_cluster_set_cloud_secure_connection_bundle_no_ssl_lib_init(CassCluster* cluster,
                                                                const char* path);

/**
 * Same as cass_cluster_set_cloud_secure_connection_bundle_no_ssl_lib_init(),
 * but with lengths for string parameters.
 *
 * @see cass_cluster_set_cloud_secure_connection_bundle_no_ssl_lib_init()
 *
 * @param[in] cluster
 * @param[in] path Absolute path to DBaaS credentials file.
 * @param[in] path_length Length of path variable.
 * @return CASS_OK if successful, otherwise error occured.
 */
CASS_EXPORT CassError
cass_cluster_set_cloud_secure_connection_bundle_no_ssl_lib_init_n(CassCluster* cluster,
                                                                  const char* path,
                                                                  size_t path_length);

/**
 * Sets the amount of time between monitor reporting event messages.
 *
 * <b>Default:</b> 300 seconds.
 *
 * @public @memberof CassCluster
 *
 * @param[in] cluster
 * @param[in] interval_secs Use 0 to disable monitor reporting event messages.
 */
CASS_EXPORT void
cass_cluster_set_monitor_reporting_interval(CassCluster* cluster,
                                            unsigned interval_secs);


/* Ancient features, which have no use nowadays. */

/**
 * Enable the <b>NO_COMPACT</b> startup option.
 *
 * This can help facilitate uninterrupted cluster upgrades where tables using
 * <b>COMPACT_STORAGE</b> will operate in "compatibility mode" for
 * <b>BATCH</b>, <b>DELETE</b>, <b>SELECT</b>, and <b>UPDATE</b> CQL operations.
 *
 * <b>Default:</b> cass_false
 *
 * @cassandra{3.0.16+}
 * @cassandra{3.11.2+}
 * @cassandra{4.0+}
 *
 * @public @memberof CassCluster
 *
 * @param[in] cluster
 * @param[in] enabled
 */
CASS_EXPORT CassError
cass_cluster_set_no_compact(CassCluster* cluster,
                            cass_bool_t enabled);


/* Functions deprecated by the CPP Driver, which would do nothing even there. */

/**
 * Sets the size of the fixed size queue that stores
 * events.
 *
 * <b>Default:</b> 8192
 *
 * @public @memberof CassCluster
 *
 * @deprecated This is no longer useful and does nothing. Expect this to be
 * removed in a future release.
 *
 * @param[in] cluster
 * @param[in] queue_size
 * @return CASS_OK if successful, otherwise an error occurred.
 */
CASS_EXPORT CASS_DEPRECATED(CassError
cass_cluster_set_queue_size_event(CassCluster* cluster,
                                  unsigned queue_size));

/**
 * Sets the maximum number of connections made to each server in each
 * IO thread.
 *
 * <b>Default:</b> 2
 *
 * @public @memberof CassCluster
 *
 * @deprecated This is no longer useful and does nothing. Expect this to be
 * removed in a future release.
 *
 * @param[in] cluster
 * @param[in] num_connections
 * @return CASS_OK if successful, otherwise an error occurred.
 */
CASS_EXPORT CASS_DEPRECATED(CassError
cass_cluster_set_max_connections_per_host(CassCluster* cluster,
                                          unsigned num_connections));

/**
 * Sets the maximum number of connections that will be created concurrently.
 * Connections are created when the current connections are unable to keep up with
 * request throughput.
 *
 * <b>Default:</b> 1
 *
 * @public @memberof CassCluster
 *
 * @deprecated This is no longer useful and does nothing. Expect this to be
 * removed in a future release.
 *
 * @param[in] cluster
 * @param[in] num_connections
 * @return CASS_OK if successful, otherwise an error occurred.
 */
CASS_EXPORT CASS_DEPRECATED(CassError
cass_cluster_set_max_concurrent_creation(CassCluster* cluster,
                                         unsigned num_connections));

/**
 * Sets the threshold for the maximum number of concurrent requests in-flight
 * on a connection before creating a new connection. The number of new connections
 * created will not exceed max_connections_per_host.
 *
 * <b>Default:</b> 100
 *
 * @public @memberof CassCluster
 *
 * @deprecated This is no longer useful and does nothing. Expect this to be
 * removed in a future release.
 *
 * @param[in] cluster
 * @param[in] num_requests
 * @return CASS_OK if successful, otherwise an error occurred.
 */
CASS_EXPORT CASS_DEPRECATED(CassError
cass_cluster_set_max_concurrent_requests_threshold(CassCluster* cluster,
                                                   unsigned num_requests));

/**
 * Sets the maximum number of requests processed by an IO worker
 * per flush.
 *
 * <b>Default:</b> 128
 *
 * @public @memberof CassCluster
 *
 * @deprecated This is no longer useful and does nothing. Expect this to be
 * removed in a future release.
 *
 * @param[in] cluster
 * @param[in] num_requests
 * @return CASS_OK if successful, otherwise an error occurred.
 */
CASS_EXPORT CASS_DEPRECATED(CassError
cass_cluster_set_max_requests_per_flush(CassCluster* cluster,
                                        unsigned num_requests));

/**
 * Sets the high water mark for the number of bytes outstanding
 * on a connection. Disables writes to a connection if the number
 * of bytes queued exceed this value.
 *
 * <b>Default:</b> 64 KB
 *
 * @public @memberof CassCluster
 *
 * @deprecated This is no longer useful and does nothing. Expect this to be
 * removed in a future release.
 *
 * @param[in] cluster
 * @param[in] num_bytes
 * @return CASS_OK if successful, otherwise an error occurred.
 */
CASS_EXPORT CASS_DEPRECATED(CassError
cass_cluster_set_write_bytes_high_water_mark(CassCluster* cluster,
                                             unsigned num_bytes));

/**
 * Sets the low water mark for number of bytes outstanding on a
 * connection. After exceeding high water mark bytes, writes will
 * only resume once the number of bytes fall below this value.
 *
 * <b>Default:</b> 32 KB
 *
 * @public @memberof CassCluster
 *
 * @deprecated This is no longer useful and does nothing. Expect this to be
 * removed in a future release.
 *
 * @param[in] cluster
 * @param[in] num_bytes
 * @return CASS_OK if successful, otherwise an error occurred.
 */
CASS_EXPORT CASS_DEPRECATED(CassError
cass_cluster_set_write_bytes_low_water_mark(CassCluster* cluster,
                                            unsigned num_bytes));

/**
 * Sets the high water mark for the number of requests queued waiting
 * for a connection in a connection pool. Disables writes to a
 * host on an IO worker if the number of requests queued exceed this
 * value.
 *
 * <b>Default:</b> 256
 *
 * @public @memberof CassCluster
 *
 * @deprecated This is no longer useful and does nothing. Expect this to be
 * removed in a future release.
 *
 * @param[in] cluster
 * @param[in] num_requests
 * @return CASS_OK if successful, otherwise an error occurred.
 */
CASS_EXPORT CASS_DEPRECATED(CassError
cass_cluster_set_pending_requests_high_water_mark(CassCluster* cluster,
                                                  unsigned num_requests));

/**
 * Sets the low water mark for the number of requests queued waiting
 * for a connection in a connection pool. After exceeding high water mark
 * requests, writes to a host will only resume once the number of requests
 * fall below this value.
 *
 * <b>Default:</b> 128
 *
 * @public @memberof CassCluster
 *
 * @deprecated This is no longer useful and does nothing. Expect this to be
 * removed in a future release.
 *
 * @param[in] cluster
 * @param[in] num_requests
 * @return CASS_OK if successful, otherwise an error occurred.
 */
CASS_EXPORT CASS_DEPRECATED(CassError
cass_cluster_set_pending_requests_low_water_mark(CassCluster* cluster,
                                                 unsigned num_requests));

/**
 * Explicitly wait for the log to flush and deallocate resources.
 * This *MUST* be the last call using the library. It is an error
 * to call any cass_*() functions after this call.
 *
 * @deprecated This is no longer useful and does nothing. Expect this to be
 * removed in a future release.
 */
CASS_EXPORT CASS_DEPRECATED(void
cass_log_cleanup());

/**
 * Sets the log queue size.
 *
 * <b>Note:</b> This needs to be done before any call that might log, such as
 * any of the cass_cluster_*() or cass_ssl_*() functions.
 *
 * <b>Default:</b> 2048
 *
 * @deprecated This is no longer useful and does nothing. Expect this to be
 * removed in a future release.
 *
 * @param[in] queue_size
 */
CASS_EXPORT CASS_DEPRECATED(void
cass_log_set_queue_size(size_t queue_size));


/* Functions incompatible with Rust Driver's architecture */

/**
 * Sets the ratio of time spent processing new requests versus handling the I/O
 * and processing of outstanding requests. The range of this setting is 1 to 100,
 * where larger values allocate more time to processing new requests and smaller
 * values allocate more time to processing outstanding requests.
 *
 * <b>Default:</b> 50
 *
 * @public @memberof CassCluster
 *
 * @param[in] cluster
 * @param[in] ratio
 * @return CASS_OK if successful, otherwise an error occurred.
 */
CASS_EXPORT CassError
cass_cluster_set_new_request_ratio(CassCluster* cluster,
                                   cass_int32_t ratio);

/**
 * Sets the maximum number of "pending write" objects that will be
 * saved for re-use for marshalling new requests. These objects may
 * hold on to a significant amount of memory and reducing the
 * number of these objects may reduce memory usage of the application.
 *
 * The cost of reducing the value of this setting is potentially slower
 * marshalling of requests prior to sending.
 *
 * <b>Default:</b> Max unsigned integer value
 *
 * @public @memberof CassCluster
 *
 * @param[in] cluster
 * @param[in] num_objects
 * @return CASS_OK if successful, otherwise an error occurred.
 */
CASS_EXPORT CassError
cass_cluster_set_max_reusable_write_objects(CassCluster* cluster,
                                            unsigned num_objects);

/**
 * Sets the size of the fixed size queue that stores
 * pending requests.
 *
 * <b>Default:</b> 8192
 *
 * @public @memberof CassCluster
 *
 * @param[in] cluster
 * @param[in] queue_size
 * @return CASS_OK if successful, otherwise an error occurred.
 */
CASS_EXPORT CassError
cass_cluster_set_queue_size_io(CassCluster* cluster,
                               unsigned queue_size);


/* Functions unimplemented on purpose */

// Binding values to unprepared statements is risky and hence
// unsupported by the Rust Driver.
/**
 * Adds a key index specifier to this a statement.
 * When using token-aware routing, this can be used to tell the driver which
 * parameters within a non-prepared, parameterized statement are part of
 * the partition key.
 *
 * Use consecutive calls for composite partition keys.
 *
 * This is not necessary for prepared statements, as the key
 * parameters are determined in the metadata processed in the prepare phase.
 *
 * @public @memberof CassStatement
 *
 * @param[in] statement
 * @param[in] index
 * @return CASS_OK if successful, otherwise an error occurred.
 */
CASS_EXPORT CassError
cass_statement_add_key_index(CassStatement* statement,
                             size_t index);

/**
 * Enable pre-preparing cached prepared statements when existing hosts become
 * available again or when new hosts are added to the cluster.
 *
 * This can help mitigate request latency when executing prepared statements
 * by avoiding an extra round trip in cases where the statement is
 * unprepared on a freshly started server. The main tradeoff is extra background
 * network traffic is required to prepare the statements on hosts as they become
 * available.
 *
 * <b>Default:</b> cass_true
 *
 * @param cluster
 * @param enabled
 * @return CASS_OK if successful, otherwise an error occurred
 */
CASS_EXPORT CassError
cass_cluster_set_prepare_on_up_or_add_host(CassCluster* cluster,
                                           cass_bool_t enabled);


/* Request tracing API
 * Explanation: the semantics is weird. CPP Driver waits for tracing info
 * to become available by performing queries to tracing tables with the
 * parameters specified by the following functions. The problem is, it does
 * not expose the results of the query; it's the user's responsibility to fetch
 * them themselves again. This is inefficient and real pain. Therefore, we
 * prefer the driver to provide no functionalities regarding server-side
 * tracing, so the user must query tracing tables themselves, than to provide
 * such a weird and incomplete semantics.
 */

 /**
  * Sets the maximum time to wait for tracing data to become available.
  *
  * <b>Default:</b> 15 milliseconds
  *
  * @param[in] cluster
  * @param[in] max_wait_time_ms
  */
 CASS_EXPORT void
 cass_cluster_set_tracing_max_wait_time(CassCluster* cluster,
                                        unsigned max_wait_time_ms);

 /**
  * Sets the amount of time to wait between attempts to check to see if tracing is
  * available.
  *
  * <b>Default:</b> 3 milliseconds
  *
  * @param[in] cluster
  * @param[in] retry_wait_time_ms
  */
 CASS_EXPORT void
 cass_cluster_set_tracing_retry_wait_time(CassCluster* cluster,
                                          unsigned retry_wait_time_ms);

 /**
  * Sets the consistency level to use for checking to see if tracing data is
  * available.
  *
  * <b>Default:</b> CASS_CONSISTENCY_ONE
  *
  * @param[in] cluster
  * @param[in] consistency
  */
 CASS_EXPORT void
 cass_cluster_set_tracing_consistency(CassCluster* cluster,
                                      CassConsistency consistency);

 /**
  * Sets a "custom" in a user defined type at the specified index.
  *
  * @public @memberof CassUserType
  *
  * @param[in] user_type
  * @param[in] index
  * @param[in] class_name
  * @param[in] value
  * @param[in] value_size
  * @return CASS_OK if successful, otherwise an error occurred.
  */
 CASS_EXPORT CassError
 cass_user_type_set_custom(CassUserType* user_type,
                           size_t index,
                           const char* class_name,
                           const cass_byte_t* value,
                           size_t value_size);

 /**
  * Same as cass_user_type_set_custom(), but with lengths for string
  * parameters.
  *
  * @public @memberof CassUserType
  *
  * @param[in] user_type
  * @param[in] index
  * @param[in] class_name
  * @param[in] class_name_length
  * @param[in] value
  * @param[in] value_size
  * @return same as cass_user_type_set_custom()
  *
  * @see cass_user_type_set_custom()
  */
 CASS_EXPORT CassError
 cass_user_type_set_custom_n(CassUserType* user_type,
                             size_t index,
                             const char* class_name,
                             size_t class_name_length,
                             const cass_byte_t* value,
                             size_t value_size);

 /**
  * Sets a "custom" in a user defined type at the specified name.
  *
  * @public @memberof CassUserType
  *
  * @param[in] user_type
  * @param[in] name
  * @param[in] class_name
  * @param[in] value
  * @param[in] value_size
  * @return CASS_OK if successful, otherwise an error occurred.
  */
 CASS_EXPORT CassError
 cass_user_type_set_custom_by_name(CassUserType* user_type,
                                   const char* name,
                                   const char* class_name,
                                   const cass_byte_t* value,
                                   size_t value_size);

 /**
  * Same as cass_user_type_set_custom_by_name(), but with lengths for string
  * parameters.
  *
  * @public @memberof CassUserType
  *
  * @param[in] user_type
  * @param[in] name
  * @param[in] name_length
  * @param[in] class_name
  * @param[in] class_name_length
  * @param[in] value
  * @param[in] value_size
  * @return same as cass_user_type_set_custom_by_name()
  *
  * @see cass_user_type_set_custom_by_name()
  */
 CASS_EXPORT CassError
 cass_user_type_set_custom_by_name_n(CassUserType* user_type,
                                     const char* name,
                                     size_t name_length,
                                     const char* class_name,
                                     size_t class_name_length,
                                     const cass_byte_t* value,
                                     size_t value_size);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* __DELETED_H_INCLUDED__ */
