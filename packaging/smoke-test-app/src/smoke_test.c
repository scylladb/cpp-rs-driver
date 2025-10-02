#include <cassandra.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef enum { SMOKE_OK = 0, SMOKE_ERR = 1 } smoke_status_t;

static void log_future_error(const CassFuture* future, const char* context) {
  const char* message = NULL;
  size_t message_length = 0;
  cass_future_error_message(future, &message, &message_length);
  fprintf(stderr, "%s: %.*s\n", context, (int)message_length,
          message ? message : "(no message)");
}

int main(int argc, char* argv[]) {
  const char* contact_point = getenv("SCYLLA_CONTACT_POINT");
  if (argc > 1 && argv[1] && strlen(argv[1]) > 0) {
    contact_point = argv[1];
  }
  if (contact_point == NULL || strlen(contact_point) == 0) {
    contact_point = "127.0.0.1";
  }

  CassCluster* cluster = cass_cluster_new();
  CassSession* session = cass_session_new();
  CassFuture* connect_future = NULL;
  smoke_status_t result = SMOKE_ERR;

  cass_cluster_set_contact_points(cluster, contact_point);

  connect_future = cass_session_connect(session, cluster);
  if (cass_future_error_code(connect_future) != CASS_OK) {
    log_future_error(connect_future, "Failed to connect to cluster");
    goto cleanup;
  }

  const char* query = "SELECT key, data_center, cluster_name FROM system.local";
  CassStatement* statement = cass_statement_new(query, 0);
  CassFuture* result_future = cass_session_execute(session, statement);

  if (cass_future_error_code(result_future) != CASS_OK) {
    log_future_error(result_future, "Failed to execute smoke-test query");
    cass_statement_free(statement);
    cass_future_free(result_future);
    goto cleanup;
  }

  const CassResult* cass_result = cass_future_get_result(result_future);
  CassIterator* rows = cass_iterator_from_result(cass_result);

  while (cass_iterator_next(rows)) {
    const CassRow* row = cass_iterator_get_row(rows);

    const CassValue* key_value = cass_row_get_column_by_name(row, "key");
    const CassValue* dc_value = cass_row_get_column_by_name(row, "data_center");
    const CassValue* cluster_value = cass_row_get_column_by_name(row, "cluster_name");

    const char* key = NULL;
    const char* data_center = NULL;
    const char* cluster_name = NULL;
    size_t key_length = 0;
    size_t dc_length = 0;
    size_t cluster_length = 0;

    if (cass_value_get_string(key_value, &key, &key_length) == CASS_OK &&
        cass_value_get_string(dc_value, &data_center, &dc_length) == CASS_OK &&
        cass_value_get_string(cluster_value, &cluster_name, &cluster_length) ==
            CASS_OK) {
      printf("system.local -> key=%.*s, data_center=%.*s, cluster_name=%.*s\n",
             (int)key_length, key ? key : "",
             (int)dc_length, data_center ? data_center : "",
             (int)cluster_length, cluster_name ? cluster_name : "");
      result = SMOKE_OK;
    } else {
      fprintf(stderr, "Unable to decode system.local row\n");
      result = SMOKE_ERR;
      break;
    }
  }

  cass_iterator_free(rows);
  cass_result_free(cass_result);
  cass_statement_free(statement);
  cass_future_free(result_future);

cleanup:
  if (connect_future != NULL) {
    cass_future_free(connect_future);
  }
  cass_session_free(session);
  cass_cluster_free(cluster);

  return result == SMOKE_OK ? EXIT_SUCCESS : EXIT_FAILURE;
}
