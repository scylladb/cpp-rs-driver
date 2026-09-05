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
  This example shows how to perform an approximate nearest neighbour (ANN)
  search - the point of the CQL `vector` type - using the CassVector API.

  It requires a ScyllaDB cluster with a running Vector Store instance, which
  serves the vector index. The CI has none, so this example is NOT run by it -
  run it by hand against a cluster of your own.

  Beware that the index is populated asynchronously, so freshly inserted
  vectors only become queryable after a while. This is why the query below
  is retried until it returns results.
*/

#include <stdio.h>
#include <stdlib.h>

#ifdef _WIN32
#include <windows.h>
#define sleep_seconds(s) Sleep((s)*1000)
#else
#include <unistd.h>
#define sleep_seconds(s) sleep(s)
#endif

#include "cassandra.h"

#define DIMENSIONS 3
#define NEIGHBOURS 2
#define MAX_ATTEMPTS 30

typedef struct {
  const char* comment;
  cass_float_t embedding[DIMENSIONS];
} Comment;

static const Comment COMMENTS[] = {
  {"the cat sat on the mat", {1.0f, 0.1f, 0.1f}},
  {"a kitten naps on a rug", {0.9f, 0.2f, 0.1f}},
  {"the stock market crashed", {0.1f, 1.0f, 0.2f}},
  {"interest rates went up", {0.2f, 0.9f, 0.1f}},
  {"a rocket launched at dawn", {0.1f, 0.1f, 1.0f}},
};

static const size_t COMMENT_COUNT = sizeof(COMMENTS) / sizeof(COMMENTS[0]);

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

/* Builds a vector<float, DIMENSIONS> out of an array of floats. */
CassVector* make_embedding(const cass_float_t* values) {
  size_t i;
  CassVector* embedding = cass_vector_new(CASS_VALUE_TYPE_FLOAT, DIMENSIONS);

  for (i = 0; i < DIMENSIONS; ++i) {
    cass_vector_set_float(embedding, i, values[i]);
  }

  return embedding;
}

CassError insert_comment(CassSession* session, const Comment* comment) {
  CassError rc = CASS_OK;
  CassFuture* future = NULL;
  CassVector* embedding = make_embedding(comment->embedding);

  const char* query =
      "INSERT INTO examples.comments (id, comment, comment_vector) VALUES (uuid(), ?, ?)";
  CassStatement* statement = cass_statement_new(query, 2);

  cass_statement_bind_string(statement, 0, comment->comment);
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

/* Returns the number of returned neighbours, or -1 on error. */
int select_nearest_comments(CassSession* session, const cass_float_t* query_values) {
  int found = -1;
  CassFuture* future = NULL;
  CassVector* query_vector = make_embedding(query_values);

  /* The ANN OF operand can be a bound parameter, just like any other vector. */
  const char* query = "SELECT comment FROM examples.comments "
                      "ORDER BY comment_vector ANN OF ? LIMIT ?";
  CassStatement* statement = cass_statement_new(query, 2);

  cass_statement_bind_vector(statement, 0, query_vector);
  cass_statement_bind_int32(statement, 1, NEIGHBOURS);

  future = cass_session_execute(session, statement);
  cass_future_wait(future);

  if (cass_future_error_code(future) != CASS_OK) {
    print_error(future);
  } else {
    const CassResult* result = cass_future_get_result(future);
    CassIterator* rows = cass_iterator_from_result(result);

    found = 0;
    while (cass_iterator_next(rows)) {
      const char* comment;
      size_t comment_length;
      const CassRow* row = cass_iterator_get_row(rows);

      cass_value_get_string(cass_row_get_column_by_name(row, "comment"), &comment,
                            &comment_length);
      printf("  %.*s\n", (int)comment_length, comment);
      ++found;
    }

    cass_result_free(result);
    cass_iterator_free(rows);
  }

  cass_future_free(future);
  cass_statement_free(statement);
  cass_vector_free(query_vector);

  return found;
}

int main(int argc, char* argv[]) {
  CassCluster* cluster = NULL;
  CassSession* session = cass_session_new();
  char* hosts = "127.0.0.1";
  size_t i;
  int attempt;
  int rc = 0;

  /* Something close to the "cat sat on the mat" comment. */
  const cass_float_t query_values[DIMENSIONS] = {0.95f, 0.15f, 0.1f};

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
      execute_query(session, "CREATE TABLE IF NOT EXISTS examples.comments ( \
                               id uuid PRIMARY KEY, \
                               comment text, \
                               comment_vector vector<float, 3>)") != CASS_OK) {
    cass_cluster_free(cluster);
    cass_session_free(session);
    return -1;
  }

  if (execute_query(session, "CREATE CUSTOM INDEX IF NOT EXISTS ann_idx \
                               ON examples.comments(comment_vector) \
                               USING 'vector_index' \
                               WITH OPTIONS = { 'similarity_function': 'COSINE' }") != CASS_OK) {
    fprintf(stderr, "Failed to create the vector index. "
                    "Is a Vector Store instance running for this cluster?\n");
    cass_cluster_free(cluster);
    cass_session_free(session);
    return -1;
  }

  for (i = 0; i < COMMENT_COUNT; ++i) {
    if (insert_comment(session, &COMMENTS[i]) != CASS_OK) {
      cass_cluster_free(cluster);
      cass_session_free(session);
      return -1;
    }
  }

  printf("Comments nearest to the query vector:\n");

  /* The index is populated asynchronously, so give it some time to catch up. */
  for (attempt = 0; attempt < MAX_ATTEMPTS; ++attempt) {
    int found = select_nearest_comments(session, query_values);

    /* The query itself failed - retrying will not help. */
    if (found < 0) {
      rc = -1;
      break;
    }

    if (found > 0) {
      break;
    }

    if (attempt + 1 == MAX_ATTEMPTS) {
      fprintf(stderr, "The vector index returned no results - is it still being built?\n");
      rc = -1;
    } else {
      sleep_seconds(1);
    }
  }

  cass_cluster_free(cluster);
  cass_session_free(session);

  return rc;
}
