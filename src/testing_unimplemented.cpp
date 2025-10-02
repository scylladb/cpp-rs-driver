#include "cassandra.h"
#include <stdexcept>

extern "C" {

CASS_EXPORT size_t cass_aggregate_meta_argument_count(const CassAggregateMeta* aggregate_meta) {
  throw std::runtime_error("UNIMPLEMENTED cass_aggregate_meta_argument_count\n");
}
CASS_EXPORT const CassDataType*
cass_aggregate_meta_argument_type(const CassAggregateMeta* aggregate_meta, size_t index) {
  throw std::runtime_error("UNIMPLEMENTED cass_aggregate_meta_argument_type\n");
}
CASS_EXPORT const CassValue*
cass_aggregate_meta_field_by_name(const CassAggregateMeta* aggregate_meta, const char* name) {
  throw std::runtime_error("UNIMPLEMENTED cass_aggregate_meta_field_by_name\n");
}
CASS_EXPORT const CassFunctionMeta*
cass_aggregate_meta_final_func(const CassAggregateMeta* aggregate_meta) {
  throw std::runtime_error("UNIMPLEMENTED cass_aggregate_meta_final_func\n");
}
CASS_EXPORT void cass_aggregate_meta_full_name(const CassAggregateMeta* aggregate_meta,
                                               const char** full_name, size_t* full_name_length) {
  throw std::runtime_error("UNIMPLEMENTED cass_aggregate_meta_full_name\n");
}
CASS_EXPORT const CassValue*
cass_aggregate_meta_init_cond(const CassAggregateMeta* aggregate_meta) {
  throw std::runtime_error("UNIMPLEMENTED cass_aggregate_meta_init_cond\n");
}
CASS_EXPORT void cass_aggregate_meta_name(const CassAggregateMeta* aggregate_meta,
                                          const char** name, size_t* name_length) {
  throw std::runtime_error("UNIMPLEMENTED cass_aggregate_meta_name\n");
}
CASS_EXPORT const CassDataType*
cass_aggregate_meta_return_type(const CassAggregateMeta* aggregate_meta) {
  throw std::runtime_error("UNIMPLEMENTED cass_aggregate_meta_return_type\n");
}
CASS_EXPORT const CassFunctionMeta*
cass_aggregate_meta_state_func(const CassAggregateMeta* aggregate_meta) {
  throw std::runtime_error("UNIMPLEMENTED cass_aggregate_meta_state_func\n");
}
CASS_EXPORT const CassDataType*
cass_aggregate_meta_state_type(const CassAggregateMeta* aggregate_meta) {
  throw std::runtime_error("UNIMPLEMENTED cass_aggregate_meta_state_type\n");
}
CASS_EXPORT void cass_authenticator_set_error(CassAuthenticator* auth, const char* message) {
  throw std::runtime_error("UNIMPLEMENTED cass_authenticator_set_error\n");
}
CASS_EXPORT CassError cass_batch_set_keyspace(CassBatch* batch, const char* keyspace) {
  throw std::runtime_error("UNIMPLEMENTED cass_batch_set_keyspace\n");
}
CASS_EXPORT CassError cass_cluster_set_authenticator_callbacks(
    CassCluster* cluster, const CassAuthenticatorCallbacks* exchange_callbacks,
    CassAuthenticatorDataCleanupCallback cleanup_callback, void* data) {
  throw std::runtime_error("UNIMPLEMENTED cass_cluster_set_authenticator_callbacks\n");
}
CASS_EXPORT CassError cass_cluster_set_cloud_secure_connection_bundle_no_ssl_lib_init(
    CassCluster* cluster, const char* path) {
  throw std::runtime_error(
      "UNIMPLEMENTED cass_cluster_set_cloud_secure_connection_bundle_no_ssl_lib_init\n");
}
CASS_EXPORT void cass_cluster_set_constant_reconnect(CassCluster* cluster, cass_uint64_t delay_ms) {
  throw std::runtime_error("UNIMPLEMENTED cass_cluster_set_constant_reconnect\n");
}
CASS_EXPORT CassError cass_cluster_set_host_listener_callback(CassCluster* cluster,
                                                              CassHostListenerCallback callback,
                                                              void* data) {
  throw std::runtime_error("UNIMPLEMENTED cass_cluster_set_host_listener_callback\n");
}
CASS_EXPORT CassError cass_cluster_set_no_compact(CassCluster* cluster, cass_bool_t enabled) {
  throw std::runtime_error("UNIMPLEMENTED cass_cluster_set_no_compact\n");
}
CASS_EXPORT CassError cass_cluster_set_prepare_on_all_hosts(CassCluster* cluster,
                                                            cass_bool_t enabled) {
  throw std::runtime_error("UNIMPLEMENTED cass_cluster_set_prepare_on_all_hosts\n");
}
CASS_EXPORT CassError cass_cluster_set_prepare_on_up_or_add_host(CassCluster* cluster,
                                                                 cass_bool_t enabled) {
  throw std::runtime_error("UNIMPLEMENTED cass_cluster_set_prepare_on_up_or_add_host\n");
}
CASS_EXPORT CassError cass_collection_append_custom(CassCollection* collection,
                                                    const char* class_name,
                                                    const cass_byte_t* value, size_t value_size) {
  throw std::runtime_error("UNIMPLEMENTED cass_collection_append_custom\n");
}
CASS_EXPORT const CassValue* cass_column_meta_field_by_name(const CassColumnMeta* column_meta,
                                                            const char* name) {
  throw std::runtime_error("UNIMPLEMENTED cass_column_meta_field_by_name\n");
}
CASS_EXPORT void cass_custom_payload_free(CassCustomPayload* payload) {
  throw std::runtime_error("UNIMPLEMENTED cass_custom_payload_free\n");
}
CASS_EXPORT void cass_custom_payload_set(CassCustomPayload* payload, const char* name,
                                         const cass_byte_t* value, size_t value_size) {
  throw std::runtime_error("UNIMPLEMENTED cass_custom_payload_set\n");
}
CASS_EXPORT CassError cass_function_meta_argument(const CassFunctionMeta* function_meta,
                                                  size_t index, const char** name,
                                                  size_t* name_length, const CassDataType** type) {
  throw std::runtime_error("UNIMPLEMENTED cass_function_meta_argument\n");
}
CASS_EXPORT size_t cass_function_meta_argument_count(const CassFunctionMeta* function_meta) {
  throw std::runtime_error("UNIMPLEMENTED cass_function_meta_argument_count\n");
}
CASS_EXPORT const CassDataType*
cass_function_meta_argument_type_by_name(const CassFunctionMeta* function_meta, const char* name) {
  throw std::runtime_error("UNIMPLEMENTED cass_function_meta_argument_type_by_name\n");
}
CASS_EXPORT void cass_function_meta_body(const CassFunctionMeta* function_meta, const char** body,
                                         size_t* body_length) {
  throw std::runtime_error("UNIMPLEMENTED cass_function_meta_body\n");
}
CASS_EXPORT cass_bool_t
cass_function_meta_called_on_null_input(const CassFunctionMeta* function_meta) {
  throw std::runtime_error("UNIMPLEMENTED cass_function_meta_called_on_null_input\n");
}
CASS_EXPORT const CassValue* cass_function_meta_field_by_name(const CassFunctionMeta* function_meta,
                                                              const char* name) {
  throw std::runtime_error("UNIMPLEMENTED cass_function_meta_field_by_name\n");
}
CASS_EXPORT void cass_function_meta_full_name(const CassFunctionMeta* function_meta,
                                              const char** full_name, size_t* full_name_length) {
  throw std::runtime_error("UNIMPLEMENTED cass_function_meta_full_name\n");
}
CASS_EXPORT void cass_function_meta_language(const CassFunctionMeta* function_meta,
                                             const char** language, size_t* language_length) {
  throw std::runtime_error("UNIMPLEMENTED cass_function_meta_language\n");
}
CASS_EXPORT void cass_function_meta_name(const CassFunctionMeta* function_meta, const char** name,
                                         size_t* name_length) {
  throw std::runtime_error("UNIMPLEMENTED cass_function_meta_name\n");
}
CASS_EXPORT const CassDataType*
cass_function_meta_return_type(const CassFunctionMeta* function_meta) {
  throw std::runtime_error("UNIMPLEMENTED cass_function_meta_return_type\n");
}
CASS_EXPORT const CassValue* cass_index_meta_field_by_name(const CassIndexMeta* index_meta,
                                                           const char* name) {
  throw std::runtime_error("UNIMPLEMENTED cass_index_meta_field_by_name\n");
}
CASS_EXPORT const CassAggregateMeta*
cass_keyspace_meta_aggregate_by_name(const CassKeyspaceMeta* keyspace_meta, const char* name,
                                     const char* arguments) {
  throw std::runtime_error("UNIMPLEMENTED cass_keyspace_meta_aggregate_by_name\n");
}
CASS_EXPORT const CassValue* cass_keyspace_meta_field_by_name(const CassKeyspaceMeta* keyspace_meta,
                                                              const char* name) {
  throw std::runtime_error("UNIMPLEMENTED cass_keyspace_meta_field_by_name\n");
}
CASS_EXPORT const CassFunctionMeta*
cass_keyspace_meta_function_by_name(const CassKeyspaceMeta* keyspace_meta, const char* name,
                                    const char* arguments) {
  throw std::runtime_error("UNIMPLEMENTED cass_keyspace_meta_function_by_name\n");
}
CASS_EXPORT cass_bool_t cass_keyspace_meta_is_virtual(const CassKeyspaceMeta* keyspace_meta) {
  throw std::runtime_error("UNIMPLEMENTED cass_keyspace_meta_is_virtual\n");
}
CASS_EXPORT const CassValue*
cass_materialized_view_meta_field_by_name(const CassMaterializedViewMeta* view_meta,
                                          const char* name) {
  throw std::runtime_error("UNIMPLEMENTED cass_materialized_view_meta_field_by_name\n");
}
CASS_EXPORT CassVersion cass_schema_meta_version(const CassSchemaMeta* schema_meta) {
  throw std::runtime_error("UNIMPLEMENTED cass_schema_meta_version\n");
}
CASS_EXPORT void
cass_session_get_speculative_execution_metrics(const CassSession* session,
                                               CassSpeculativeExecutionMetrics* output) {
  throw std::runtime_error("UNIMPLEMENTED cass_session_get_speculative_execution_metrics\n");
}
CASS_EXPORT CassError cass_statement_add_key_index(CassStatement* statement, size_t index) {
  throw std::runtime_error("UNIMPLEMENTED cass_statement_add_key_index\n");
}
CASS_EXPORT CassError cass_statement_bind_custom(CassStatement* statement, size_t index,
                                                 const char* class_name, const cass_byte_t* value,
                                                 size_t value_size) {
  throw std::runtime_error("UNIMPLEMENTED cass_statement_bind_custom\n");
}
CASS_EXPORT CassError cass_statement_bind_custom_by_name(CassStatement* statement, const char* name,
                                                         const char* class_name,
                                                         const cass_byte_t* value,
                                                         size_t value_size) {
  throw std::runtime_error("UNIMPLEMENTED cass_statement_bind_custom_by_name\n");
}
CASS_EXPORT CassError cass_statement_set_custom_payload(CassStatement* statement,
                                                        const CassCustomPayload* payload) {
  throw std::runtime_error("UNIMPLEMENTED cass_statement_set_custom_payload\n");
}
CASS_EXPORT CassError cass_statement_set_keyspace(CassStatement* statement, const char* keyspace) {
  throw std::runtime_error("UNIMPLEMENTED cass_statement_set_keyspace\n");
}
CASS_EXPORT CassClusteringOrder
cass_table_meta_clustering_key_order(const CassTableMeta* table_meta, size_t index) {
  throw std::runtime_error("UNIMPLEMENTED cass_table_meta_clustering_key_order\n");
}
CASS_EXPORT const CassValue* cass_table_meta_field_by_name(const CassTableMeta* table_meta,
                                                           const char* name) {
  throw std::runtime_error("UNIMPLEMENTED cass_table_meta_field_by_name\n");
}
CASS_EXPORT const CassIndexMeta* cass_table_meta_index_by_name(const CassTableMeta* table_meta,
                                                               const char* index) {
  throw std::runtime_error("UNIMPLEMENTED cass_table_meta_index_by_name\n");
}
CASS_EXPORT size_t cass_table_meta_index_count(const CassTableMeta* table_meta) {
  throw std::runtime_error("UNIMPLEMENTED cass_table_meta_index_count\n");
}
CASS_EXPORT cass_bool_t cass_table_meta_is_virtual(const CassTableMeta* table_meta) {
  throw std::runtime_error("UNIMPLEMENTED cass_table_meta_is_virtual\n");
}
CASS_EXPORT CassError cass_tuple_set_custom(CassTuple* tuple, size_t index, const char* class_name,
                                            const cass_byte_t* value, size_t value_size) {
  throw std::runtime_error("UNIMPLEMENTED cass_tuple_set_custom\n");
}
CASS_EXPORT CassError cass_user_type_set_custom(CassUserType* user_type, size_t index,
                                                const char* class_name, const cass_byte_t* value,
                                                size_t value_size) {
  throw std::runtime_error("UNIMPLEMENTED cass_user_type_set_custom\n");
}
CASS_EXPORT CassError cass_user_type_set_custom_by_name(CassUserType* user_type, const char* name,
                                                        const char* class_name,
                                                        const cass_byte_t* value,
                                                        size_t value_size) {
  throw std::runtime_error("UNIMPLEMENTED cass_user_type_set_custom_by_name\n");
}

} // extern "C"
