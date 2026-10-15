/*
  Copyright (c) 2026 ScyllaDB Ltd.

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

  http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
*/

#include "integration.hpp"

class SelectWithoutFromTests : public Integration {
public:
  SelectWithoutFromTests()
      : is_select_without_from_supported_(false) {}

  void SetUp() {
    Integration::SetUp();
    CHECK_FAILURE;

    // Detect the backing table independently of the SELECT syntax under test.
    Result result =
        session_.execute("SELECT * FROM system.one_row", CASS_CONSISTENCY_LOCAL_ONE, false, false);
    if (result.error_code() == CASS_ERROR_SERVER_INVALID_QUERY &&
        result.error_message().find("unconfigured table") != std::string::npos &&
        result.error_message().find("one_row") != std::string::npos) {
      return;
    }
    ASSERT_EQ(CASS_OK, result.error_code())
        << result.error_description() << ": " << result.error_message();
    is_select_without_from_supported_ = true;
  }

  void connect(Cluster cluster) {
    // Connect without creating or selecting the integration fixture's keyspace.
    cluster_ = cluster;
    session_ = cluster.connect();
    CHECK_FAILURE;
  }

  void TearDown() {
    // This fixture creates no schema and never stops or pauses cluster nodes.
  }

protected:
  bool is_select_without_from_supported_;
};

namespace {

const uint64_t CLOCK_SKEW_TOLERANCE_MS = 5000;

void assert_single_column(Result result, const std::string& name, CassValueType value_type) {
  ASSERT_EQ(CASS_OK, result.error_code())
      << result.error_description() << ": " << result.error_message();
  ASSERT_TRUE(result.get() != NULL);
  ASSERT_EQ(1u, cass_result_row_count(result.get()));
  ASSERT_EQ(1u, cass_result_column_count(result.get()));

  const char* column_name = NULL;
  size_t column_name_length = 0;
  ASSERT_EQ(CASS_OK, cass_result_column_name(result.get(), 0, &column_name, &column_name_length));
  EXPECT_EQ(name, std::string(column_name, column_name_length));
  EXPECT_EQ(value_type, cass_result_column_type(result.get(), 0));

  const CassDataType* data_type = cass_result_column_data_type(result.get(), 0);
  ASSERT_TRUE(data_type != NULL);
  EXPECT_EQ(value_type, cass_data_type_type(data_type));
}

void assert_literal_result(Result result) {
  ASSERT_NO_FATAL_FAILURE(assert_single_column(result, "one", CASS_VALUE_TYPE_INT));
  const CassRow* row = cass_result_first_row(result.get());
  ASSERT_TRUE(row != NULL);
  const CassValue* value = cass_row_get_column(row, 0);
  ASSERT_TRUE(value != NULL);
  ASSERT_FALSE(cass_value_is_null(value));
  EXPECT_EQ(CASS_VALUE_TYPE_INT, cass_value_type(value));
  cass_int32_t decoded = 0;
  ASSERT_EQ(CASS_OK, cass_value_get_int32(value, &decoded));
  EXPECT_EQ(1, decoded);
}

void assert_now_result(Result result, uint64_t before_ms, uint64_t after_ms) {
  ASSERT_NO_FATAL_FAILURE(assert_single_column(result, "t", CASS_VALUE_TYPE_TIMEUUID));
  const CassRow* row = cass_result_first_row(result.get());
  ASSERT_TRUE(row != NULL);
  const CassValue* value = cass_row_get_column(row, 0);
  ASSERT_TRUE(value != NULL);
  ASSERT_FALSE(cass_value_is_null(value));
  EXPECT_EQ(CASS_VALUE_TYPE_TIMEUUID, cass_value_type(value));
  CassUuid decoded;
  ASSERT_EQ(CASS_OK, cass_value_get_uuid(value, &decoded));
  EXPECT_EQ(1u, cass_uuid_version(decoded));
  const uint64_t timestamp_ms = cass_uuid_timestamp(decoded);
  EXPECT_GE(timestamp_ms, before_ms - CLOCK_SKEW_TOLERANCE_MS);
  EXPECT_LE(timestamp_ms, after_ms + CLOCK_SKEW_TOLERANCE_MS);
}

} // namespace

/**
 * Executes literal and function selectors without creating or selecting a keyspace or table.
 *
 * Both tests skip only when the independent system.one_row probe reports an unconfigured table.
 * All other probe errors and all SELECT without FROM errors fail, including syntax errors.
 * This fixture does not exercise routing based on user-table metadata.
 *
 * @test_category queries
 * @expected_result Aliased column metadata and decoded integer/timeuuid values are correct.
 */
CASSANDRA_INTEGRATION_TEST_F(SelectWithoutFromTests, Simple) {
  CHECK_FAILURE;
  if (!is_select_without_from_supported_) {
    SKIP_TEST("Server does not support SELECT without FROM");
  }

  ASSERT_NO_FATAL_FAILURE(assert_literal_result(
      session_.execute("SELECT 1 AS one", CASS_CONSISTENCY_LOCAL_ONE, false, false)));

  const uint64_t before_ms = time_since_epoch_in_ms();
  Result result = session_.execute("SELECT now() AS t", CASS_CONSISTENCY_LOCAL_ONE, false, false);
  const uint64_t after_ms = time_since_epoch_in_ms();
  ASSERT_NO_FATAL_FAILURE(assert_now_result(result, before_ms, after_ms));
}

/**
 * Prepares literal and function selectors without a keyspace or user table, binding a LIMIT value.
 *
 * Uses the same independent support probe and skip rules as the simple-statement test.
 *
 * @test_category prepared
 * @expected_result Binding succeeds and result metadata and decoded values match simple execution.
 */
CASSANDRA_INTEGRATION_TEST_F(SelectWithoutFromTests, Prepared) {
  CHECK_FAILURE;
  if (!is_select_without_from_supported_) {
    SKIP_TEST("Server does not support SELECT without FROM");
  }

  Prepared prepared = session_.prepare("SELECT 1 AS one LIMIT ?", false);
  ASSERT_EQ(CASS_OK, prepared.error_code())
      << prepared.error_description() << ": " << prepared.error_message();
  ASSERT_TRUE(prepared.get() != NULL);
  Statement statement = prepared.bind();
  ASSERT_EQ(CASS_OK, cass_statement_bind_int32(statement.get(), 0, 1));
  ASSERT_NO_FATAL_FAILURE(assert_literal_result(session_.execute(statement, false)));

  prepared = session_.prepare("SELECT now() AS t LIMIT ?", false);
  ASSERT_EQ(CASS_OK, prepared.error_code())
      << prepared.error_description() << ": " << prepared.error_message();
  ASSERT_TRUE(prepared.get() != NULL);
  statement = prepared.bind();
  ASSERT_EQ(CASS_OK, cass_statement_bind_int32(statement.get(), 0, 1));
  const uint64_t before_ms = time_since_epoch_in_ms();
  Result result = session_.execute(statement, false);
  const uint64_t after_ms = time_since_epoch_in_ms();
  ASSERT_NO_FATAL_FAILURE(assert_now_result(result, before_ms, after_ms));
}
