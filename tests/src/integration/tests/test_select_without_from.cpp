/*
  Copyright (c) DataStax, Inc.

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
  void connect(Cluster cluster) {
    // Connect without creating or selecting the integration fixture's keyspace.
    cluster_ = cluster;
    session_ = cluster.connect();
    CHECK_FAILURE;
  }

  void TearDown() {
    // This fixture creates no schema and never stops or pauses cluster nodes.
  }
};

namespace {

bool is_select_without_from_unsupported(CassError error_code) {
  return error_code == CASS_ERROR_SERVER_SYNTAX_ERROR;
}

void assert_single_column(Result result, const std::string& name, CassValueType value_type) {
  ASSERT_EQ(CASS_OK, result.error_code());
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
  ASSERT_NO_FATAL_FAILURE(assert_single_column(result, "1", CASS_VALUE_TYPE_INT));
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

void assert_now_result(Result result) {
  ASSERT_NO_FATAL_FAILURE(assert_single_column(result, "system.now()", CASS_VALUE_TYPE_TIMEUUID));
  const CassRow* row = cass_result_first_row(result.get());
  ASSERT_TRUE(row != NULL);
  const CassValue* value = cass_row_get_column(row, 0);
  ASSERT_TRUE(value != NULL);
  ASSERT_FALSE(cass_value_is_null(value));
  EXPECT_EQ(CASS_VALUE_TYPE_TIMEUUID, cass_value_type(value));
  CassUuid decoded;
  ASSERT_EQ(CASS_OK, cass_value_get_uuid(value, &decoded));
  EXPECT_EQ(1u, cass_uuid_version(decoded));
  EXPECT_NE(0u, cass_uuid_timestamp(decoded));
}

} // namespace

CASSANDRA_INTEGRATION_TEST_F(SelectWithoutFromTests, SimpleAndPrepared) {
  CHECK_FAILURE;

  Result result = session_.execute("SELECT 1", CASS_CONSISTENCY_LOCAL_ONE, false, false);
  if (is_select_without_from_unsupported(result.error_code())) {
    SKIP_TEST("Server does not support SELECT without FROM");
  }
  ASSERT_EQ(CASS_OK, result.error_code());

  ASSERT_NO_FATAL_FAILURE(assert_literal_result(result));
  ASSERT_NO_FATAL_FAILURE(assert_now_result(session_.execute("SELECT now()")));

  Prepared prepared = session_.prepare("SELECT 1");
  ASSERT_EQ(CASS_OK, prepared.error_code());
  ASSERT_TRUE(prepared.get() != NULL);
  ASSERT_NO_FATAL_FAILURE(assert_literal_result(session_.execute(prepared.bind())));

  prepared = session_.prepare("SELECT now()");
  ASSERT_EQ(CASS_OK, prepared.error_code());
  ASSERT_TRUE(prepared.get() != NULL);
  ASSERT_NO_FATAL_FAILURE(assert_now_result(session_.execute(prepared.bind())));
}
