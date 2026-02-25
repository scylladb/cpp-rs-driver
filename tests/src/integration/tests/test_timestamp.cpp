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

extern "C" {
#include "testing_rust_impls.h"
}

class TimestampTests : public Integration {
public:
  void SetUp() {
    Integration::SetUp();
    session_.execute(
        format_string(CASSANDRA_KEY_VALUE_TABLE_FORMAT, table_name_.c_str(), "text", "text"));
    prepared_insert_statement_ = session_.prepare(
        format_string(CASSANDRA_KEY_VALUE_INSERT_FORMAT, table_name_.c_str(), "?", "?"));
  }

  Text generate_key() { return Text(uuid_generator_.generate_random_uuid().str()); }

  Statement create_insert_statement(Text key) {
    Statement insert_statement = prepared_insert_statement_.bind();
    insert_statement.bind<Text>(0, key);
    insert_statement.bind<Text>(1, key);
    return insert_statement;
  }

  BigInteger select_timestamp(Text key) {
    Statement select_statement(
        format_string("SELECT writetime(value) AS write_time_value, value FROM %s WHERE key=%s",
                      table_name_.c_str(), "?"),
        1);
    select_statement.bind<Text>(0, key);
    Result result = session_.execute(select_statement);
    return result.first_row().column_by_name<BigInteger>("write_time_value");
  }

private:
  Prepared prepared_insert_statement_;
};

/**
 * Set timestamp on the insert statement and validate the assigned timestamp.
 *
 * @since 2.1.0
 * @jira_ticket CPP-266
 * @cassandra_version 2.1.x
 */
CASSANDRA_INTEGRATION_TEST_F(TimestampTests, Statement) {
  CHECK_FAILURE;
  SKIP_IF_CASSANDRA_VERSION_LT(2.1.0);

  Text key(generate_key());
  Statement insert_statement(create_insert_statement(key));
  insert_statement.set_timestamp(1234);
  session_.execute(insert_statement);

  EXPECT_EQ(BigInteger(1234), select_timestamp(key));
}

/**
 * Set timestamp on the batch statement and validate the assigned timestamp.
 *
 * @since 2.1.0
 * @jira_ticket CPP-266
 * @cassandra_version 2.1.x
 */
CASSANDRA_INTEGRATION_TEST_F(TimestampTests, BatchStatement) {
  CHECK_FAILURE;
  SKIP_IF_CASSANDRA_VERSION_LT(2.1.0);

  Batch batch_statement;
  std::vector<Text> keys;
  for (int i = 0; i < 2; ++i) {
    keys.push_back(generate_key());
    batch_statement.add(create_insert_statement(keys.back()));
  }
  batch_statement.set_timestamp(1234);
  session_.execute(batch_statement);

  for (std::vector<Text>::iterator it = keys.begin(), end = keys.end(); it != end; ++it) {
    EXPECT_EQ(BigInteger(1234), select_timestamp(*it));
  }
}

/**
 * Verifies that the server side timestamp generator is used on a statement and validate the
 * assigned timestamp from the generator.
 *
 * @since 2.1.0
 * @jira_ticket CPP-266
 * @cassandra_version 2.1.x
 */
CASSANDRA_INTEGRATION_TEST_F(TimestampTests, ServerSideTimestampGeneratorStatement) {
  CHECK_FAILURE;
  SKIP_IF_CASSANDRA_VERSION_LT(2.1.0);
  ServerSideTimestampGenerator generator;
  connect(default_cluster().with_timestamp_generator(generator));

  Text key(generate_key());
  BigInteger expected_timestamp(static_cast<int64_t>(time_since_epoch_us()));
  session_.execute(create_insert_statement(key));

  EXPECT_NEAR(static_cast<double>(expected_timestamp.value()),
              static_cast<double>(select_timestamp(key).value()),
              static_cast<double>(1e+6)); // 1 second error/tolerance
}

/**
 * Verifies that the server side timestamp generator is used on a batch statement and validate the
 * assigned timestamp from the generator.
 *
 * @since 2.1.0
 * @jira_ticket CPP-266
 * @cassandra_version 2.1.x
 */
CASSANDRA_INTEGRATION_TEST_F(TimestampTests, ServerSideTimestampGeneratorBatchStatement) {
  CHECK_FAILURE;
  SKIP_IF_CASSANDRA_VERSION_LT(2.1.0);
  ServerSideTimestampGenerator generator;
  connect(default_cluster().with_timestamp_generator(generator));

  Batch batch_statement;
  std::vector<Text> keys;
  for (int i = 0; i < 2; ++i) {
    keys.push_back(generate_key());
    batch_statement.add(create_insert_statement(keys.back()));
  }
  BigInteger expected_timestamp(static_cast<int64_t>(time_since_epoch_us()));
  session_.execute(batch_statement);

  BigInteger last_timestamp;
  for (std::vector<Text>::iterator it = keys.begin(), end = keys.end(); it != end; ++it) {
    BigInteger timestamp(select_timestamp(*it));
    EXPECT_NEAR(static_cast<double>(expected_timestamp.value()),
                static_cast<double>(timestamp.value()),
                static_cast<double>(1e+6)); // 1 second error/tolerance

    if (!last_timestamp.is_null()) { // All timestamps in the batch should be equal
      EXPECT_EQ(timestamp, last_timestamp);
    }
    last_timestamp = timestamp;
  }
}

/**
 * Verifies that the monotonic timestamp generator is used and validates the assigned timestamp from
 * the generator.
 *
 * @since 2.6.0
 * @jira_ticket CPP-412
 * @cassandra_version 2.1.x
 */
CASSANDRA_INTEGRATION_TEST_F(TimestampTests, MonotonicTimestampGenerator) {
  CHECK_FAILURE;
  SKIP_IF_CASSANDRA_VERSION_LT(2.1.0);
  TimestampGenerator generator(testing_timestamp_gen_monotonic_new());
  connect(default_cluster().with_timestamp_generator(generator));

  BigInteger last_timestamp;
  for (int i = 0; i < 100; ++i) {
    Text key(generate_key());
    session_.execute(create_insert_statement(key));

    BigInteger timestamp(select_timestamp(key));
    EXPECT_TRUE(testing_timestamp_gen_contains_timestamp(generator.get(), timestamp.value()));

    if (!last_timestamp.is_null()) {
      EXPECT_NE(last_timestamp, timestamp);
      EXPECT_GT(timestamp, last_timestamp); // Monotonic timestamps should be always increasing
    }
    last_timestamp = timestamp;
  }
}
