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

#include <vector>

/**
 * Wraps a raw `CassValue*`, so that it can be retrieved from a row.
 *
 * A vector is not a collection, and it is not part of the typed value framework
 * (see `values/`), so we work with the C API directly here.
 */
class RawValue {
public:
  RawValue(const CassValue* value)
      : value_(value) {}

  const CassValue* get() const { return value_; }

private:
  const CassValue* value_;
};

class VectorTests : public Integration {
public:
  VectorTests() { is_schema_metadata_ = true; }

  void SetUp() {
    // Vectors require a server that supports them.
    SKIP_IF_CASSANDRA_VERSION_LT(5.0.0);
    Integration::SetUp();
  }

protected:
  /**
   * Creates a table with a single vector column of the given CQL type.
   */
  void create_vector_table(const std::string& cql_type) {
    session_.execute(format_string("CREATE TABLE IF NOT EXISTS %s (key int PRIMARY KEY, value %s)",
                                   table_name_.c_str(), cql_type.c_str()));
  }

  /**
   * Reads back the single vector value of the row with the given key.
   */
  const CassValue* select_vector(Result& result, int key) {
    result = session_.execute(
        format_string("SELECT value FROM %s WHERE key=%d", table_name_.c_str(), key));
    return result.first_row().column_by_name<RawValue>("value").get();
  }

  /**
   * Asserts that the value is a vector of the expected type and dimensions.
   */
  void assert_vector_metadata(const CassValue* value, CassValueType element_type,
                              size_t dimensions) {
    ASSERT_FALSE(cass_value_is_null(value));
    ASSERT_EQ(CASS_VALUE_TYPE_VECTOR, cass_value_type(value));
    ASSERT_EQ(element_type, cass_value_primary_sub_type(value));
    ASSERT_EQ(dimensions, cass_value_item_count(value));

    // A vector is not a collection, so the collection API must reject it.
    ASSERT_FALSE(cass_value_is_collection(value));
    ASSERT_TRUE(cass_iterator_from_collection(value) == NULL);
  }
};

/**
 * Insert and read back a vector of a fixed-size element type.
 *
 * Elements of a fixed-size type are written to the wire with no length prefix
 * at all, so this exercises that encoding.
 *
 * @jira_ticket DRIVER-386
 * @test_category data_types:vector
 * @expected_result The vector is read back exactly as it was bound.
 */
CASSANDRA_INTEGRATION_TEST_F(VectorTests, FixedSizeElements) {
  CHECK_FAILURE;

  const cass_float_t expected[] = { 1.0f, -2.5f, 0.0f, 42.25f };
  const size_t dimensions = sizeof(expected) / sizeof(expected[0]);

  create_vector_table("vector<float, 4>");

  Statement insert(format_string("INSERT INTO %s (key, value) VALUES (0, ?)",
                                 table_name_.c_str()),
                   1);
  CassVector* vector = cass_vector_new(CASS_VALUE_TYPE_FLOAT, dimensions);
  for (size_t i = 0; i < dimensions; ++i) {
    ASSERT_EQ(CASS_OK, cass_vector_set_float(vector, i, expected[i]));
  }
  ASSERT_EQ(CASS_OK, cass_statement_bind_vector(insert.get(), 0, vector));
  cass_vector_free(vector);
  session_.execute(insert);

  Result result;
  const CassValue* value = select_vector(result, 0);
  assert_vector_metadata(value, CASS_VALUE_TYPE_FLOAT, dimensions);

  CassIterator* iterator = cass_iterator_from_vector(value);
  ASSERT_TRUE(iterator != NULL);
  for (size_t i = 0; i < dimensions; ++i) {
    ASSERT_TRUE(cass_iterator_next(iterator));
    cass_float_t element;
    ASSERT_EQ(CASS_OK, cass_value_get_float(cass_iterator_get_value(iterator), &element));
    EXPECT_EQ(expected[i], element);
  }
  EXPECT_FALSE(cass_iterator_next(iterator));
  cass_iterator_free(iterator);
}

/**
 * Insert and read back a vector of a variable-size element type.
 *
 * Elements of a variable-size type are prefixed with an unsigned vint length,
 * which is a different encoding than the one used for fixed-size elements.
 * One of the values below is long enough to need a multi-byte vint.
 *
 * @jira_ticket DRIVER-386
 * @test_category data_types:vector
 * @expected_result The vector is read back exactly as it was bound.
 */
CASSANDRA_INTEGRATION_TEST_F(VectorTests, VariableSizeElements) {
  CHECK_FAILURE;

  // Note that an element may not be empty: the server cannot tell an empty
  // value apart from a null one, and vectors do not accept nulls.
  std::vector<std::string> expected;
  expected.push_back("a");
  expected.push_back("alpha");
  // Long enough to need more than one byte of vint length.
  expected.push_back(std::string(300, 'b'));

  create_vector_table("vector<text, 3>");

  Statement insert(format_string("INSERT INTO %s (key, value) VALUES (0, ?)",
                                 table_name_.c_str()),
                   1);
  CassVector* vector = cass_vector_new(CASS_VALUE_TYPE_TEXT, expected.size());
  for (size_t i = 0; i < expected.size(); ++i) {
    ASSERT_EQ(CASS_OK, cass_vector_set_string(vector, i, expected[i].c_str()));
  }
  ASSERT_EQ(CASS_OK, cass_statement_bind_vector(insert.get(), 0, vector));
  cass_vector_free(vector);
  session_.execute(insert);

  Result result;
  const CassValue* value = select_vector(result, 0);
  assert_vector_metadata(value, CASS_VALUE_TYPE_VARCHAR, expected.size());

  CassIterator* iterator = cass_iterator_from_vector(value);
  ASSERT_TRUE(iterator != NULL);
  for (size_t i = 0; i < expected.size(); ++i) {
    ASSERT_TRUE(cass_iterator_next(iterator));
    const char* element;
    size_t element_length;
    ASSERT_EQ(CASS_OK,
              cass_value_get_string(cass_iterator_get_value(iterator), &element, &element_length));
    EXPECT_EQ(expected[i], std::string(element, element_length));
  }
  EXPECT_FALSE(cass_iterator_next(iterator));
  cass_iterator_free(iterator);
}

/**
 * Bind a vector to a prepared statement, whose bound values are typechecked
 * against the schema metadata of the table.
 *
 * @jira_ticket DRIVER-386
 * @test_category data_types:vector
 * @test_category queries:prepared
 * @expected_result The vector is read back exactly as it was bound, and a
 * vector of a mismatched type is rejected.
 */
CASSANDRA_INTEGRATION_TEST_F(VectorTests, Prepared) {
  CHECK_FAILURE;

  const cass_float_t expected[] = { 0.5f, 1.5f, 2.5f };
  const size_t dimensions = sizeof(expected) / sizeof(expected[0]);

  create_vector_table("vector<float, 3>");

  Prepared prepared = session_.prepare(
      format_string("INSERT INTO %s (key, value) VALUES (?, ?)", table_name_.c_str()));
  Statement insert = prepared.bind();
  insert.bind<Integer>(0, Integer(0));

  CassVector* vector = cass_vector_new(CASS_VALUE_TYPE_FLOAT, dimensions);
  for (size_t i = 0; i < dimensions; ++i) {
    ASSERT_EQ(CASS_OK, cass_vector_set_float(vector, i, expected[i]));
  }
  ASSERT_EQ(CASS_OK, cass_statement_bind_vector(insert.get(), 1, vector));
  cass_vector_free(vector);
  session_.execute(insert);

  // A vector of the wrong element type does not typecheck against the metadata.
  Statement bad_insert = prepared.bind();
  bad_insert.bind<Integer>(0, Integer(1));
  CassVector* bad_vector = cass_vector_new(CASS_VALUE_TYPE_DOUBLE, dimensions);
  for (size_t i = 0; i < dimensions; ++i) {
    ASSERT_EQ(CASS_OK, cass_vector_set_double(bad_vector, i, 1.0));
  }
  EXPECT_EQ(CASS_ERROR_LIB_INVALID_VALUE_TYPE,
            cass_statement_bind_vector(bad_insert.get(), 1, bad_vector));
  cass_vector_free(bad_vector);

  // A vector of the wrong number of dimensions does not typecheck either.
  CassVector* short_vector = cass_vector_new(CASS_VALUE_TYPE_FLOAT, dimensions - 1);
  for (size_t i = 0; i < dimensions - 1; ++i) {
    ASSERT_EQ(CASS_OK, cass_vector_set_float(short_vector, i, 1.0f));
  }
  EXPECT_EQ(CASS_ERROR_LIB_INVALID_VALUE_TYPE,
            cass_statement_bind_vector(bad_insert.get(), 1, short_vector));
  cass_vector_free(short_vector);

  Result result;
  const CassValue* value = select_vector(result, 0);
  assert_vector_metadata(value, CASS_VALUE_TYPE_FLOAT, dimensions);

  CassIterator* iterator = cass_iterator_from_vector(value);
  ASSERT_TRUE(iterator != NULL);
  for (size_t i = 0; i < dimensions; ++i) {
    ASSERT_TRUE(cass_iterator_next(iterator));
    cass_float_t element;
    ASSERT_EQ(CASS_OK, cass_value_get_float(cass_iterator_get_value(iterator), &element));
    EXPECT_EQ(expected[i], element);
  }
  cass_iterator_free(iterator);
}

/**
 * Verify that a vector column is reported as such in the schema metadata,
 * with the correct element type and number of dimensions.
 *
 * @jira_ticket DRIVER-386
 * @test_category data_types:vector
 * @test_category metadata
 * @expected_result The column data type is a vector of 3 floats.
 */
CASSANDRA_INTEGRATION_TEST_F(VectorTests, SchemaMetadata) {
  CHECK_FAILURE;

  create_vector_table("vector<float, 3>");

  Schema schema = session_.schema();
  Table table = schema.keyspace(keyspace_name_).table(table_name_);
  ASSERT_TRUE(table);

  const CassDataType* data_type =
      cass_column_meta_data_type(cass_table_meta_column_by_name(table.get(), "value"));
  ASSERT_TRUE(data_type != NULL);
  EXPECT_EQ(CASS_VALUE_TYPE_VECTOR, cass_data_type_type(data_type));
  EXPECT_EQ(1u, cass_data_type_sub_type_count(data_type));
  EXPECT_EQ(CASS_VALUE_TYPE_FLOAT,
            cass_data_type_type(cass_data_type_sub_data_type(data_type, 0)));

  size_t dimensions = 0;
  ASSERT_EQ(CASS_OK, cass_data_type_vector_dimensions(data_type, &dimensions));
  EXPECT_EQ(3u, dimensions);

  // A vector built from that very data type can be bound to the column.
  CassVector* vector = cass_vector_new_from_data_type(data_type);
  ASSERT_TRUE(vector != NULL);
  for (size_t i = 0; i < dimensions; ++i) {
    ASSERT_EQ(CASS_OK, cass_vector_set_float(vector, i, static_cast<cass_float_t>(i)));
  }
  cass_vector_free(vector);
}
