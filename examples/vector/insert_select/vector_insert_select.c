/*
  This is free and unencumbered software released into the public domain.

  Anyone is free to copy, modify, publish, use, compile, sell, or
  distribute this software, either in source code form or as a compiled
  binary, for any purpose, commercial or non-commercial, and by any
  means.

  In jurisdictions that recognize copyright laws, the author or authors
  of this software dedicate any and all copyright interest in the
  software to the public domain. We make this dedication for the benefit
  of the public at large and to the detriment of our heirs and
  successors. We intend this dedication to be an overt act of
  relinquishment in perpetuity of all present and future rights to this
  software under copyright law.

  THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
  EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
  MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
  IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR
  OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
  ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
  OTHER DEALINGS IN THE SOFTWARE.

  For more information, please refer to <http://unlicense.org/>
*/

/*
  This example shows how to insert and read back values of the CQL `vector`
  type, using the CassVector API.

  It requires a ScyllaDB version that supports the vector type. It is NOT run
  by the CI - run it by hand against a cluster of your own.
*/

#include <stdio.h>
#include <stdlib.h>

#include "cassandra.h"

#define DIMENSIONS 3

void print_error(CassFuture* future) {
  const char* message;
  size_t message_length;
  cass_future_error_message(future, &message, &message_length);
  fprintf(stderr, "Error: %.*s\n", (int)message_length, message);
}

CassCluster* create_cluster(const char* hosts) {
  CassCluster* cluster = cass_cluster_new();
  cass_cluster_set_contact_points(cluster, hosts);
  return cluster;
}

CassError connect_session(CassSession* session, const CassCluster* cluster) {
  CassError rc = CASS_OK;
  CassFuture* future = cass_session_connect(session, cluster);

  cass_future_wait(future);
  rc = cass_future_error_code(future);
  if (rc != CASS_OK) {
    print_error(future);
  }
  cass_future_free(future);

  return rc;
}

CassError execute_query(CassSession* session, const char* query) {
  CassError rc = CASS_OK;
  CassFuture* future = NULL;
  CassStatement* statement = cass_statement_new(query, 0);

  future = cass_session_execute(session, statement);
  cass_future_wait(future);

  rc = cass_future_error_code(future);
  if (rc != CASS_OK) {
    print_error(future);
  }

  cass_future_free(future);
  cass_statement_free(statement);

  return rc;
}

CassError insert_into_vector(CassSession* session, cass_int32_t id, const cass_float_t* values) {
  CassError rc = CASS_OK;
  CassStatement* statement = NULL;
  CassFuture* future = NULL;
  CassVector* embedding = NULL;
  size_t i;

  const char* query = "INSERT INTO examples.vectors (id, embedding) VALUES (?, ?)";

  statement = cass_statement_new(query, 2);

  /* A vector is always fully typed: both the type of its elements and the
     number of dimensions are required upfront. */
  embedding = cass_vector_new(CASS_VALUE_TYPE_FLOAT, DIMENSIONS);

  /* All the elements have to be set - a vector element cannot be null. */
  for (i = 0; i < DIMENSIONS; ++i) {
    cass_vector_set_float(embedding, i, values[i]);
  }

  cass_statement_bind_int32(statement, 0, id);
  cass_statement_bind_vector(statement, 1, embedding);

  future = cass_session_execute(session, statement);
  cass_future_wait(future);

  rc = cass_future_error_code(future);
  if (rc != CASS_OK) {
    print_error(future);
  }

  cass_future_free(future);
  cass_statement_free(statement);
  cass_vector_free(embedding);

  return rc;
}

CassError select_from_vector(CassSession* session) {
  CassError rc = CASS_OK;
  CassStatement* statement = NULL;
  CassFuture* future = NULL;

  const char* query = "SELECT id, embedding FROM examples.vectors";

  statement = cass_statement_new(query, 0);

  future = cass_session_execute(session, statement);
  cass_future_wait(future);

  rc = cass_future_error_code(future);
  if (rc != CASS_OK) {
    print_error(future);
  } else {
    const CassResult* result = cass_future_get_result(future);
    CassIterator* rows = cass_iterator_from_result(result);

    while (cass_iterator_next(rows)) {
      cass_int32_t id;
      const CassRow* row = cass_iterator_get_row(rows);
      const CassValue* id_value = cass_row_get_column_by_name(row, "id");
      const CassValue* embedding_value = cass_row_get_column_by_name(row, "embedding");

      /* Note that a vector is not a collection, so it has an iterator of its
         own - cass_iterator_from_collection() does not accept it. */
      CassIterator* embedding = cass_iterator_from_vector(embedding_value);

      cass_value_get_int32(id_value, &id);
      printf("id %d: [", id);

      while (cass_iterator_next(embedding)) {
        cass_float_t element;
        cass_value_get_float(cass_iterator_get_value(embedding), &element);
        printf(" %f", element);
      }

      printf(" ]\n");

      cass_iterator_free(embedding);
    }

    cass_result_free(result);
    cass_iterator_free(rows);
  }

  cass_future_free(future);
  cass_statement_free(statement);

  return rc;
}

int main(int argc, char* argv[]) {
  CassCluster* cluster = NULL;
  CassSession* session = cass_session_new();
  char* hosts = "127.0.0.1";
  int rc = 0;

  const cass_float_t first[DIMENSIONS] = {0.1f, 0.2f, 0.3f};
  const cass_float_t second[DIMENSIONS] = {8.0f, 2.3f, 58.0f};

  if (argc > 1) {
    hosts = argv[1];
  }
  cluster = create_cluster(hosts);

  if (connect_session(session, cluster) != CASS_OK) {
    cass_cluster_free(cluster);
    cass_session_free(session);
    return -1;
  }

  if (execute_query(session, "CREATE KEYSPACE IF NOT EXISTS examples WITH replication = { \
                               'class': 'NetworkTopologyStrategy', 'replication_factor': '1' }") !=
          CASS_OK ||
      execute_query(session, "CREATE TABLE IF NOT EXISTS examples.vectors (id int PRIMARY KEY, \
                               embedding vector<float, 3>)") != CASS_OK ||
      insert_into_vector(session, 1, first) != CASS_OK ||
      insert_into_vector(session, 2, second) != CASS_OK ||
      select_from_vector(session) != CASS_OK) {
    rc = -1;
  }

  cass_cluster_free(cluster);
  cass_session_free(session);

  return rc;
}
