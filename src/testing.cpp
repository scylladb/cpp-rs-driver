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

#include "testing.hpp"

#include "get_time.hpp"
#include "murmur3.hpp"
#include <cstring>
#include <sstream>

extern "C" {
#include "testing_rust_impls.h"
}

namespace datastax { namespace internal { namespace testing {

String get_host_from_future(CassFuture* future) {
  char* host;
  size_t host_length;

  testing_future_get_host(future, &host, &host_length);

  if (host == nullptr) {
    throw std::runtime_error("CassFuture returned a null host string.");
  }

  std::string host_str(host, host_length);
  OStringStream ss;
  ss << host_str;

  testing_free_cstring(host);

  return ss.str();
}

StringVec get_attempted_hosts_from_future(CassFuture* future) {
  // Original implementation commented out for reference.
  /*
  if (future->type() != Future::FUTURE_TYPE_RESPONSE) {
      return StringVec();
  }

  StringVec attempted_hosts;
  ResponseFuture* response_future = static_cast<ResponseFuture*>(future->from());

  AddressVec attempted_addresses = response_future->attempted_addresses();
  for (const auto& address : attempted_addresses) {
      attempted_hosts.push_back(address.to_string());
  }
  std::sort(attempted_hosts.begin(), attempted_hosts.end());
  return attempted_hosts;
  */

  StringVec attempted_hosts;

  char* const concatenated_attempted_addresses = testing_future_get_attempted_hosts(future);

  std::string concatenated_addresses = concatenated_attempted_addresses;
  std::stringstream stream{ concatenated_addresses };
  while (true) {
    String address;
    if (std::getline(stream, address, '\n')) {
      if (!address.empty()) {
        attempted_hosts.push_back(address);
      }
    } else {
      break; // No more addresses to read.
    }
  }
  std::sort(attempted_hosts.begin(), attempted_hosts.end());

  testing_free_cstring(concatenated_attempted_addresses);

  return attempted_hosts;
}

unsigned get_connect_timeout_from_cluster(CassCluster* cluster) {
  return testing_cluster_get_connect_timeout(cluster);
}

int get_port_from_cluster(CassCluster* cluster) { return testing_cluster_get_port(cluster); }

String get_contact_points_from_cluster(CassCluster* cluster) {
  char* contact_points;
  size_t contact_points_length;
  testing_cluster_get_contact_points(cluster, &contact_points, &contact_points_length);

  if (contact_points == nullptr) {
    throw std::runtime_error("CassCluster returned a null contact points string.\
                              This means that one of the contact points contained a nul byte in it.");
  }

  std::string contact_points_str(contact_points, contact_points_length);
  OStringStream ss;
  ss << contact_points_str;

  testing_free_cstring(contact_points);

  return ss.str();
}

int64_t create_murmur3_hash_from_string(const String& value) {
  return MurmurHash3_x64_128(value.data(), value.size(), 0);
}

uint64_t get_time_since_epoch_in_ms() { return internal::get_time_since_epoch_ms(); }

uint64_t get_host_latency_average(CassSession* session, String ip_address, int port) {
  throw std::runtime_error("Unimplemented 'get_host_latency_average'!");
}

CassConsistency get_consistency(const CassStatement* statement) {
  throw std::runtime_error("Unimplemented 'get_consistency'!");
}

CassConsistency get_serial_consistency(const CassStatement* statement) {
  throw std::runtime_error("Unimplemented 'get_serial_consistency'!");
}

uint64_t get_request_timeout_ms(const CassStatement* statement) {
  throw std::runtime_error("Unimplemented 'get_request_timeout_ms'!");
}

const CassRetryPolicy* get_retry_policy(const CassStatement* statement) {
  throw std::runtime_error("Unimplemented 'get_retry_policy'!");
}

String get_server_name(CassFuture* future) {
  throw std::runtime_error("Unimplemented 'get_server_name'!");
}

void set_record_attempted_hosts(CassStatement* statement, bool enable) {
  testing_statement_set_recording_history_listener(statement, (cass_bool_t)enable);
}

void set_sleeping_history_listener_on_statement(CassStatement* statement, uint64_t sleep_time_ms) {
  testing_statement_set_sleeping_history_listener(statement, sleep_time_ms);
}

void set_sleeping_history_listener_on_batch(CassBatch* batch, uint64_t sleep_time_ms) {
  testing_batch_set_sleeping_history_listener(batch, sleep_time_ms);
}

CassRetryPolicy* retry_policy_ignoring_new() { return testing_retry_policy_ignoring_new(); }

}}} // namespace datastax::internal::testing
