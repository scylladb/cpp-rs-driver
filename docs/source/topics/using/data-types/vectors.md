# Vectors

A vector is a fixed-size sequence of values of the same type, written
`vector<type, dimensions>` in CQL. It exists for vector search: an
[ANN](https://cloud.docs.scylladb.com/stable/vector-search/) (approximate
nearest neighbour) query orders rows by the distance between a vector column
and a query vector.

Despite looking like one, a vector is **not** a CQL collection:

* the number of dimensions is part of its type, not of its value;
* its elements cannot be null;
* it can only be updated as a whole - there is no element-wise update;
* it is not created by [`cass_collection_new()`] and it is not accepted by
  [`cass_iterator_from_collection()`]; [`cass_value_is_collection()`] returns
  `cass_false` for it.

This is why vectors have a type of their own in the API, [`CassVector`].

## Creating a Vector

A vector is always fully typed: the wire representation of its elements depends
on their type, so both the element type and the number of dimensions have to be
given upfront.

```c
/* A vector<float, 3> */
CassVector* embedding = cass_vector_new(CASS_VALUE_TYPE_FLOAT, 3);

/* Elements are set by position, and they are typechecked against the element
 * type - this would return an error, as the elements are floats
 */
CassError rc = cass_vector_set_double(embedding, 0, 0.1);
assert(rc != CASS_OK);

cass_vector_set_float(embedding, 0, 0.1f);
cass_vector_set_float(embedding, 1, 0.2f);
cass_vector_set_float(embedding, 2, 0.3f);

/* ... */

/* Vectors must be freed */
cass_vector_free(embedding);
```

**All elements must be set.** A vector's elements cannot be null, so there is no
`cass_vector_set_null()`, and binding a vector with an element that was never
set fails when the statement is serialized.

[`cass_vector_new()`] only accepts native element types. For a vector whose
elements are a UDT, a tuple, a collection or another vector, build the data type
and use [`cass_vector_new_from_data_type()`]:

```c
/* A vector<frozen<list<int>>, 2> */
CassDataType* list_type = cass_data_type_new(CASS_VALUE_TYPE_LIST);
cass_data_type_add_sub_value_type(list_type, CASS_VALUE_TYPE_INT);

CassDataType* vector_type = cass_data_type_new_vector(list_type, 2);
CassVector* vector = cass_vector_new_from_data_type(vector_type);

/* ... */

cass_vector_free(vector);
cass_data_type_free(vector_type);
cass_data_type_free(list_type);
```

The number of dimensions of a vector data type can be read back with
[`cass_data_type_vector_dimensions()`], and its element type is its only
sub-type, available through `cass_data_type_sub_data_type()`.

## Binding a Vector

```c
CassStatement* statement =
  cass_statement_new("INSERT INTO examples.vectors (id, embedding) VALUES (?, ?)", 2);

cass_statement_bind_int32(statement, 0, 1);
cass_statement_bind_vector(statement, 1, embedding);

/* ... */
```

When the statement is prepared, the bound vector is typechecked against the
column's data type, so a vector of the wrong element type or of the wrong number
of dimensions is rejected before the request is sent.

Vectors can also be nested in other values, using
[`cass_collection_append_vector()`], [`cass_tuple_set_vector()`],
[`cass_user_type_set_vector()`] and [`cass_vector_set_vector()`].

## Consuming values from a Vector result

A vector is read with an iterator of its own,
[`cass_iterator_from_vector()`]. The number of elements is available from
[`cass_value_item_count()`] - it comes from the type, as a vector carries no
element count on the wire.

```c
void iterate_vector(const CassRow* row) {
  /* Retrieve the vector value from the column */
  const CassValue* vector_value = cass_row_get_column_by_name(row, "embedding");

  /* A vector is not a collection: cass_iterator_from_collection() would
   * return NULL here
   */
  CassIterator* vector_iterator = cass_iterator_from_vector(vector_value);

  while (cass_iterator_next(vector_iterator)) {
    cass_float_t element;
    cass_value_get_float(cass_iterator_get_value(vector_iterator), &element);

    /* ... */
  }

  /* The vector iterator needs to be freed */
  cass_iterator_free(vector_iterator);
}
```

## Vector search

Once a vector index exists on a vector column, the column can be searched by
similarity. The query vector is bound like any other vector:

```cql
CREATE CUSTOM INDEX ann_idx ON examples.comments(comment_vector)
  USING 'vector_index' WITH OPTIONS = { 'similarity_function': 'COSINE' };
```

```c
CassStatement* statement =
  cass_statement_new("SELECT comment FROM examples.comments "
                     "ORDER BY comment_vector ANN OF ? LIMIT ?", 2);

cass_statement_bind_vector(statement, 0, query_vector);
cass_statement_bind_int32(statement, 1, 5);

/* ... */
```

Note that the index is populated asynchronously, so freshly inserted vectors
only become queryable after a while.

A vector index is served by a Vector Store instance, which has to be running
alongside the cluster. The `examples/vector` directory contains two runnable
examples: `insert_select`, which only needs a server supporting the vector type,
and `search_ann`, which needs a Vector Store as well.

[`CassVector`]: https://cpp-rs-driver.docs.scylladb.com/stable/api/struct.CassVector
[`cass_vector_new()`]: https://cpp-rs-driver.docs.scylladb.com/stable/api/struct.CassVector#cass-vector-new
[`cass_vector_new_from_data_type()`]: https://cpp-rs-driver.docs.scylladb.com/stable/api/struct.CassVector#cass-vector-new-from-data-type
[`cass_vector_set_vector()`]: https://cpp-rs-driver.docs.scylladb.com/stable/api/struct.CassVector#cass-vector-set-vector
[`cass_data_type_vector_dimensions()`]: https://cpp-rs-driver.docs.scylladb.com/stable/api/struct.CassDataType#cass-data-type-vector-dimensions
[`cass_collection_new()`]: https://cpp-rs-driver.docs.scylladb.com/stable/api/struct.CassCollection#cass-collection-new
[`cass_collection_append_vector()`]: https://cpp-rs-driver.docs.scylladb.com/stable/api/struct.CassCollection#cass-collection-append-vector
[`cass_tuple_set_vector()`]: https://cpp-rs-driver.docs.scylladb.com/stable/api/struct.CassTuple#cass-tuple-set-vector
[`cass_user_type_set_vector()`]: https://cpp-rs-driver.docs.scylladb.com/stable/api/struct.CassUserType#cass-user-type-set-vector
[`cass_iterator_from_collection()`]: https://cpp-rs-driver.docs.scylladb.com/stable/api/struct.CassIterator#cass-iterator-from-collection
[`cass_iterator_from_vector()`]: https://cpp-rs-driver.docs.scylladb.com/stable/api/struct.CassIterator#cass-iterator-from-vector
[`cass_value_is_collection()`]: https://cpp-rs-driver.docs.scylladb.com/stable/api/struct.CassValue#cass-value-is-collection
[`cass_value_item_count()`]: https://cpp-rs-driver.docs.scylladb.com/stable/api/struct.CassValue#cass-value-item-count
