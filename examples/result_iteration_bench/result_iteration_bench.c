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
 * Benchmark for result iteration performance.
 *
 * Creates a table with 10 columns, inserts 10000 rows, then performs
 * an unpaged SELECT * and measures how long it takes to iterate over
 * all rows in the result set. The iteration is repeated multiple times
 * to get stable measurements.
 */

#define _GNU_SOURCE
#include <assert.h>
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "cassandra.h"

/*
 * Allocation tracking via interposition.
 * We override malloc/realloc/free to count calls, forwarding
 * to the real implementations via dlsym(RTLD_NEXT, ...).
 */

static size_t g_malloc_count = 0;
static size_t g_realloc_count = 0;
static size_t g_free_count = 0;

typedef void* (*real_malloc_t)(size_t);
typedef void* (*real_realloc_t)(void*, size_t);
typedef void (*real_free_t)(void*);

static real_malloc_t real_malloc = NULL;
static real_realloc_t real_realloc = NULL;
static real_free_t real_free = NULL;

/* Small static buffer to bootstrap dlsym (which may call malloc internally). */
static char bootstrap_buf[4096];
static size_t bootstrap_used = 0;
static int resolving = 0;

static void ensure_real_funcs(void) {
  if (real_malloc != NULL) return;
  resolving = 1;
  real_malloc = (real_malloc_t)dlsym(RTLD_NEXT, "malloc");
  real_realloc = (real_realloc_t)dlsym(RTLD_NEXT, "realloc");
  real_free = (real_free_t)dlsym(RTLD_NEXT, "free");
  resolving = 0;
}

void* malloc(size_t size) {
  if (resolving) {
    /* dlsym is resolving symbols and called malloc - use static buffer. */
    size_t aligned = (size + 7) & ~(size_t)7; /* align to 8 */
    if (bootstrap_used + aligned > sizeof(bootstrap_buf)) {
      return NULL;
    }
    void* ptr = bootstrap_buf + bootstrap_used;
    bootstrap_used += aligned;
    return ptr;
  }
  ensure_real_funcs();
  g_malloc_count++;
  return real_malloc(size);
}

void* realloc(void* ptr, size_t size) {
  if (resolving) {
    size_t aligned = (size + 7) & ~(size_t)7;
    if (bootstrap_used + aligned > sizeof(bootstrap_buf)) {
      return NULL;
    }
    size_t old_bootstrap_used = bootstrap_used;
    void* new_ptr = bootstrap_buf + bootstrap_used;
    bootstrap_used += aligned;
    /* Copy old data if ptr is in the bootstrap buffer. */
    if (ptr != NULL && ptr >= (void*)bootstrap_buf &&
        ptr < (void*)(bootstrap_buf + sizeof(bootstrap_buf))) {
      size_t old_offset = (char*)ptr - bootstrap_buf;
      size_t old_avail = old_bootstrap_used - old_offset;
      size_t copy_size = old_avail < size ? old_avail : size;
      memcpy(new_ptr, ptr, copy_size);
    }
    return new_ptr;
  }
  ensure_real_funcs();
  g_realloc_count++;
  return real_realloc(ptr, size);
}

void free(void* ptr) {
  /* Ignore frees into our bootstrap buffer. */
  if (ptr >= (void*)bootstrap_buf && ptr < (void*)(bootstrap_buf + sizeof(bootstrap_buf))) {
    return;
  }
  if (resolving) return;
  ensure_real_funcs();
  g_free_count++;
  real_free(ptr);
}

#define NUM_ROWS 10000
#define NUM_INSERT_CONCURRENT 500
#define NUM_BENCH_ITERATIONS 20

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

void insert_rows(CassSession* session) {
  const char* query = "INSERT INTO result_iter_bench.bench_table "
                      "(pk, col1, col2, col3, col4, col5, col6, col7, col8, col9) "
                      "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

  int total_sent = 0;

  while (total_sent < NUM_ROWS) {
    int batch_size = NUM_ROWS - total_sent;
    int i;
    CassFuture** futures;

    if (batch_size > NUM_INSERT_CONCURRENT) {
      batch_size = NUM_INSERT_CONCURRENT;
    }

    futures = (CassFuture**)malloc(sizeof(CassFuture*) * batch_size);

    for (i = 0; i < batch_size; ++i) {
      int row_id = total_sent + i;
      char str_buf[64];
      CassStatement* statement = cass_statement_new(query, 10);

      cass_statement_bind_int32(statement, 0, row_id);

      sprintf(str_buf, "value_%d_col1", row_id);
      cass_statement_bind_string(statement, 1, str_buf);

      sprintf(str_buf, "value_%d_col2", row_id);
      cass_statement_bind_string(statement, 2, str_buf);

      cass_statement_bind_int32(statement, 3, row_id * 10);
      cass_statement_bind_int64(statement, 4, (cass_int64_t)row_id * 100);
      cass_statement_bind_float(statement, 5, (cass_float_t)row_id * 1.1f);
      cass_statement_bind_double(statement, 6, (cass_double_t)row_id * 2.2);
      cass_statement_bind_bool(statement, 7, row_id % 2 == 0 ? cass_true : cass_false);
      cass_statement_bind_int32(statement, 8, row_id + 1000);
      cass_statement_bind_int64(statement, 9, (cass_int64_t)row_id + 2000);

      futures[i] = cass_session_execute(session, statement);
      cass_statement_free(statement);
    }

    for (i = 0; i < batch_size; ++i) {
      CassError rc = cass_future_error_code(futures[i]);
      if (rc != CASS_OK) {
        print_error(futures[i]);
      }
      cass_future_free(futures[i]);
    }

    free(futures);
    total_sent += batch_size;
  }

  printf("Inserted %d rows.\n", total_sent);
}

/*
 * Executes an unpaged SELECT * and returns the result.
 * The caller is responsible for freeing the result.
 * Returns NULL on error.
 */
const CassResult* execute_select_all(CassSession* session) {
  const CassResult* result = NULL;
  CassError rc = CASS_OK;
  CassFuture* future = NULL;
  CassStatement* statement = cass_statement_new("SELECT * FROM result_iter_bench.bench_table", 0);

  /* Disable paging to get all rows in a single result set. */
  cass_statement_set_paging_size(statement, -1);

  future = cass_session_execute(session, statement);
  cass_future_wait(future);

  rc = cass_future_error_code(future);
  if (rc != CASS_OK) {
    print_error(future);
  } else {
    result = cass_future_get_result(future);
  }

  cass_future_free(future);
  cass_statement_free(statement);

  return result;
}

/*
 * Iterates over all rows in the result, reading every column value
 * to simulate realistic consumption. Returns the number of rows iterated.
 * Frees the result when done.
 */
size_t iterate_result(const CassResult* result) {
  size_t row_count = 0;
  CassIterator* iterator = cass_iterator_from_result(result);

  while (cass_iterator_next(iterator)) {
    const CassRow* row = cass_iterator_get_row(iterator);

    /* Read all columns to realistically exercise the iteration path. */
    cass_int32_t pk;
    const char* col1;
    size_t col1_len;
    const char* col2;
    size_t col2_len;
    cass_int32_t col3;
    cass_int64_t col4;
    cass_float_t col5;
    cass_double_t col6;
    cass_bool_t col7;
    cass_int32_t col8;
    cass_int64_t col9;

    cass_value_get_int32(cass_row_get_column(row, 0), &pk);
    cass_value_get_string(cass_row_get_column(row, 1), &col1, &col1_len);
    cass_value_get_string(cass_row_get_column(row, 2), &col2, &col2_len);
    cass_value_get_int32(cass_row_get_column(row, 3), &col3);
    cass_value_get_int64(cass_row_get_column(row, 4), &col4);
    cass_value_get_float(cass_row_get_column(row, 5), &col5);
    cass_value_get_double(cass_row_get_column(row, 6), &col6);
    cass_value_get_bool(cass_row_get_column(row, 7), &col7);
    cass_value_get_int32(cass_row_get_column(row, 8), &col8);
    cass_value_get_int64(cass_row_get_column(row, 9), &col9);

    row_count++;
  }

  cass_iterator_free(iterator);
  cass_result_free(result);

  return row_count;
}

double time_diff_ms(struct timespec* start, struct timespec* end) {
  double sec = (double)(end->tv_sec - start->tv_sec);
  double nsec = (double)(end->tv_nsec - start->tv_nsec);
  return sec * 1000.0 + nsec / 1000000.0;
}

int compare_double(const void* a, const void* b) {
  double da = *(const double*)a;
  double db = *(const double*)b;
  if (da < db) return -1;
  if (da > db) return 1;
  return 0;
}

int main(int argc, char* argv[]) {
  int i;
  double total_ms = 0.0;
  double min_ms, max_ms, median_ms;
  double timings[NUM_BENCH_ITERATIONS];
  CassCluster* cluster = NULL;
  CassSession* session = cass_session_new();
  char* hosts = "127.0.0.1";
  if (argc > 1) {
    hosts = argv[1];
  }
  cluster = create_cluster(hosts);

  if (connect_session(session, cluster) != CASS_OK) {
    cass_cluster_free(cluster);
    cass_session_free(session);
    return -1;
  }

  /* Set up schema. */
  execute_query(session, "CREATE KEYSPACE IF NOT EXISTS result_iter_bench WITH replication = { "
                         "'class': 'SimpleStrategy', 'replication_factor': '1' }");

  execute_query(session, "DROP TABLE IF EXISTS result_iter_bench.bench_table");

  execute_query(session, "CREATE TABLE result_iter_bench.bench_table ("
                         "pk int PRIMARY KEY, "
                         "col1 text, "
                         "col2 text, "
                         "col3 int, "
                         "col4 bigint, "
                         "col5 float, "
                         "col6 double, "
                         "col7 boolean, "
                         "col8 int, "
                         "col9 bigint)");

  /* Insert rows. */
  printf("Inserting %d rows...\n", NUM_ROWS);
  insert_rows(session);

  /* Warm-up: run one iteration before timing to warm up caches. */
  {
    const CassResult* result = execute_select_all(session);
    assert(result != NULL);
    size_t warmup_count = iterate_result(result);
    printf("Warm-up: iterated over %zu rows.\n", warmup_count);
    assert(warmup_count == NUM_ROWS);
  }

  /* Benchmark: only the result iteration is timed, not query execution. */
  printf("\nRunning %d benchmark iterations (timing iteration only)...\n\n", NUM_BENCH_ITERATIONS);

  for (i = 0; i < NUM_BENCH_ITERATIONS; ++i) {
    struct timespec start, end;
    size_t row_count;
    double elapsed_ms;
    const CassResult* result;
    size_t malloc_before, realloc_before, free_before;

    /* Execute query and wait for result (not timed). */
    result = execute_select_all(session);
    assert(result != NULL);

    /* Snapshot alloc counters, then time only the iteration. */
    malloc_before = g_malloc_count;
    realloc_before = g_realloc_count;
    free_before = g_free_count;

    clock_gettime(CLOCK_MONOTONIC, &start);
    row_count = iterate_result(result);
    clock_gettime(CLOCK_MONOTONIC, &end);

    assert(row_count == NUM_ROWS);

    elapsed_ms = time_diff_ms(&start, &end);
    timings[i] = elapsed_ms;
    total_ms += elapsed_ms;

    printf("  Iteration %2d: %zu rows in %.3f ms  |  mallocs: %zu  reallocs: %zu  frees: %zu\n",
           i + 1, row_count, elapsed_ms, g_malloc_count - malloc_before,
           g_realloc_count - realloc_before, g_free_count - free_before);
  }

  /* Compute statistics. */
  qsort(timings, NUM_BENCH_ITERATIONS, sizeof(double), compare_double);
  min_ms = timings[0];
  max_ms = timings[NUM_BENCH_ITERATIONS - 1];
  if (NUM_BENCH_ITERATIONS % 2 == 0) {
    median_ms = (timings[NUM_BENCH_ITERATIONS / 2 - 1] + timings[NUM_BENCH_ITERATIONS / 2]) / 2.0;
  } else {
    median_ms = timings[NUM_BENCH_ITERATIONS / 2];
  }

  printf("\n--- Benchmark Results (%d rows x %d iterations) ---\n", NUM_ROWS, NUM_BENCH_ITERATIONS);
  printf("  Min:    %.3f ms\n", min_ms);
  printf("  Max:    %.3f ms\n", max_ms);
  printf("  Median: %.3f ms\n", median_ms);
  printf("  Mean:   %.3f ms\n", total_ms / NUM_BENCH_ITERATIONS);
  printf("  p95:    %.3f ms\n", timings[(int)(NUM_BENCH_ITERATIONS * 0.95) - 1]);
  printf("  Total:  %.3f ms\n", total_ms);

  /* Clean up. */
  cass_cluster_free(cluster);
  cass_session_free(session);

  return 0;
}
