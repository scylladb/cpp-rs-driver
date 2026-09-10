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

#include <algorithm>
#include <condition_variable>
#include <mutex>
#include <thread>
#include <vector>

namespace {

const size_t THREAD_COUNT = 8;
const size_t UUIDS_PER_THREAD = 1000;
const cass_uint64_t UUID_TIMESTAMP_MASK = 0x0FFFFFFFFFFFFFFFULL;

} // namespace

class UuidTests : public Integration {
public:
  UuidTests() { is_ccm_requested_ = false; }
};

/**
 * Generates time UUIDs concurrently through the public C API.
 *
 * @test_category uuid
 * @expected_result Every generated UUID has a unique, monotonically allocated timestamp.
 */
CASSANDRA_INTEGRATION_TEST_F(UuidTests, ConcurrentTimeGeneration) {
  UuidGen generator;
  CassUuidGen* const uuid_gen = generator.get();
  std::vector<CassUuid> uuids(THREAD_COUNT * UUIDS_PER_THREAD);

  std::mutex start_mutex;
  std::condition_variable start_condition;
  size_t ready_threads = 0;
  bool start = false;

  std::vector<std::thread> threads;
  threads.reserve(THREAD_COUNT);
  for (size_t thread_index = 0; thread_index < THREAD_COUNT; ++thread_index) {
    threads.push_back(std::thread([&, thread_index]() {
      {
        std::unique_lock<std::mutex> lock(start_mutex);
        ++ready_threads;
        start_condition.notify_all();
        start_condition.wait(lock, [&start]() { return start; });
      }

      const size_t offset = thread_index * UUIDS_PER_THREAD;
      for (size_t i = 0; i < UUIDS_PER_THREAD; ++i) {
        cass_uuid_gen_time(uuid_gen, &uuids[offset + i]);
      }
    }));
  }

  {
    std::unique_lock<std::mutex> lock(start_mutex);
    start_condition.wait(lock, [&ready_threads]() { return ready_threads == THREAD_COUNT; });
    start = true;
  }
  start_condition.notify_all();

  for (std::vector<std::thread>::iterator it = threads.begin(); it != threads.end(); ++it) {
    it->join();
  }

  std::vector<cass_uint64_t> timestamps;
  timestamps.reserve(uuids.size());
  for (size_t i = 0; i < uuids.size(); ++i) {
    EXPECT_EQ(1u, cass_uuid_version(uuids[i]));
    timestamps.push_back(uuids[i].time_and_version & UUID_TIMESTAMP_MASK);
  }

  for (size_t thread_index = 0; thread_index < THREAD_COUNT; ++thread_index) {
    const size_t offset = thread_index * UUIDS_PER_THREAD;
    for (size_t i = 1; i < UUIDS_PER_THREAD; ++i) {
      EXPECT_LT(uuids[offset + i - 1].time_and_version & UUID_TIMESTAMP_MASK,
                uuids[offset + i].time_and_version & UUID_TIMESTAMP_MASK);
    }
  }

  std::sort(timestamps.begin(), timestamps.end());
  EXPECT_EQ(timestamps.end(), std::unique(timestamps.begin(), timestamps.end()));
}
