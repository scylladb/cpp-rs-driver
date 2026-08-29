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

class SelectWithoutFromTests : public Integration {};

namespace {

bool is_select_without_from_unsupported(CassError error_code) {
  return error_code == CASS_ERROR_SERVER_SYNTAX_ERROR ||
         error_code == CASS_ERROR_SERVER_INVALID_QUERY;
}

void assert_single_column(Result result, const std::string& name, CassValueType value_type) {
  ASSERT_EQ(1u, result.row_count());
  ASSERT_EQ(1u, result.column_count());

  std::vector<std::string> column_names = result.column_names();
  ASSERT_EQ(1u, column_names.size());
  EXPECT_EQ(name, column_names[0]);

  const CassDataType* data_type = cass_result_column_data_type(result.get(), 0);
  ASSERT_TRUE(data_type != NULL);
  EXPECT_EQ(value_type, cass_data_type_type(data_type));
}

void assert_literal_result(Result result) {
  assert_single_column(result, "1", CASS_VALUE_TYPE_INT);

  Integer value = result.first_row().next().as<Integer>();
  ASSERT_FALSE(value.is_null());
  EXPECT_EQ(1, value.value());
}

void assert_now_result(Result result) {
  assert_single_column(result, "now()", CASS_VALUE_TYPE_TIMEUUID);

  TimeUuid value = result.first_row().next().as<TimeUuid>();
  ASSERT_FALSE(value.is_null());
  EXPECT_NE(0u, value.wrapped_value().timestamp());
}

} // namespace

CASSANDRA_INTEGRATION_TEST_F(SelectWithoutFromTests, SimpleAndPrepared) {
  CHECK_FAILURE;

  Result result = session_.execute("SELECT 1", CASS_CONSISTENCY_LOCAL_ONE, false, false);
  if (is_select_without_from_unsupported(result.error_code())) {
    SKIP_TEST("Server does not support SELECT without FROM");
  }
  ASSERT_EQ(CASS_OK, result.error_code());

  assert_literal_result(result);
  assert_now_result(session_.execute("SELECT now()"));

  Prepared prepared = session_.prepare("SELECT 1");
  assert_literal_result(session_.execute(prepared.bind()));

  prepared = session_.prepare("SELECT now()");
  assert_now_result(session_.execute(prepared.bind()));
}
