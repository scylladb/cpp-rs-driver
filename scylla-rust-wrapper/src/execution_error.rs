use crate::argconv::*;
use crate::cass_error::*;
use crate::cass_error_types::CassWriteType;
use crate::cass_types::CassConsistency;
use crate::types::*;
use scylla::deserialize::DeserializationError;
use scylla::errors::{DbError, ExecutionError, RequestAttemptError, WriteType};
use scylla::frame::frame_errors::ResultMetadataAndRowsCountParseError;
use scylla::statement::Consistency;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CassErrorResult {
    #[error(transparent)]
    Execution(#[from] ExecutionError),
    #[error(transparent)]
    ResultMetadataLazyDeserialization(#[from] ResultMetadataAndRowsCountParseError),
    #[error("Failed to deserialize first row: {0}")]
    Deserialization(#[from] DeserializationError),
}

unsafe impl FFI for CassErrorResult {
    type Origin = FromArc;
}

impl From<Consistency> for CassConsistency {
    fn from(c: Consistency) -> CassConsistency {
        match c {
            Consistency::Any => CassConsistency::CASS_CONSISTENCY_ANY,
            Consistency::One => CassConsistency::CASS_CONSISTENCY_ONE,
            Consistency::Two => CassConsistency::CASS_CONSISTENCY_TWO,
            Consistency::Three => CassConsistency::CASS_CONSISTENCY_THREE,
            Consistency::Quorum => CassConsistency::CASS_CONSISTENCY_QUORUM,
            Consistency::All => CassConsistency::CASS_CONSISTENCY_ALL,
            Consistency::LocalQuorum => CassConsistency::CASS_CONSISTENCY_LOCAL_QUORUM,
            Consistency::EachQuorum => CassConsistency::CASS_CONSISTENCY_EACH_QUORUM,
            Consistency::LocalOne => CassConsistency::CASS_CONSISTENCY_LOCAL_ONE,
            Consistency::Serial => CassConsistency::CASS_CONSISTENCY_SERIAL,
            Consistency::LocalSerial => CassConsistency::CASS_CONSISTENCY_LOCAL_SERIAL,
        }
    }
}

impl From<&WriteType> for CassWriteType {
    fn from(c: &WriteType) -> CassWriteType {
        match c {
            WriteType::Simple => CassWriteType::CASS_WRITE_TYPE_SIMPLE,
            WriteType::Batch => CassWriteType::CASS_WRITE_TYPE_BATCH,
            WriteType::UnloggedBatch => CassWriteType::CASS_WRITE_TYPE_UNLOGGED_BATCH,
            WriteType::Counter => CassWriteType::CASS_WRITE_TYPE_COUNTER,
            WriteType::BatchLog => CassWriteType::CASS_WRITE_TYPE_BATCH_LOG,
            WriteType::Cas => CassWriteType::CASS_WRITE_TYPE_CAS,
            WriteType::View => CassWriteType::CASS_WRITE_TYPE_VIEW,
            WriteType::Cdc => CassWriteType::CASS_WRITE_TYPE_CDC,
            WriteType::Other(_) => CassWriteType::CASS_WRITE_TYPE_UNKNOWN,
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_error_result_free(
    error_result: CassOwnedSharedPtr<CassErrorResult, CConst>,
) {
    ArcFFI::free(error_result);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_error_result_code(
    error_result: CassBorrowedSharedPtr<CassErrorResult, CConst>,
) -> CassError {
    let Some(error_result) = ArcFFI::as_ref(error_result) else {
        tracing::error!("Provided null error result pointer to cass_error_result_code!");
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    error_result.to_cass_error()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_error_result_consistency(
    error_result: CassBorrowedSharedPtr<CassErrorResult, CConst>,
) -> CassConsistency {
    let Some(error_result) = ArcFFI::as_ref(error_result) else {
        tracing::error!("Provided null error result pointer to cass_error_result_consistency!");
        return CassConsistency::CASS_CONSISTENCY_UNKNOWN;
    };

    match error_result {
        CassErrorResult::Execution(ExecutionError::LastAttemptError(
            RequestAttemptError::DbError(DbError::Unavailable { consistency, .. }, _)
            | RequestAttemptError::DbError(DbError::ReadTimeout { consistency, .. }, _)
            | RequestAttemptError::DbError(DbError::WriteTimeout { consistency, .. }, _)
            | RequestAttemptError::DbError(DbError::ReadFailure { consistency, .. }, _)
            | RequestAttemptError::DbError(DbError::WriteFailure { consistency, .. }, _),
        )) => CassConsistency::from(*consistency),
        _ => CassConsistency::CASS_CONSISTENCY_UNKNOWN,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_error_result_responses_received(
    error_result: CassBorrowedSharedPtr<CassErrorResult, CConst>,
) -> cass_int32_t {
    let Some(error_result) = ArcFFI::as_ref(error_result) else {
        tracing::error!(
            "Provided null error result pointer to cass_error_result_responses_received!"
        );
        return -1;
    };

    match error_result {
        CassErrorResult::Execution(ExecutionError::LastAttemptError(attempt_error)) => {
            match attempt_error {
                RequestAttemptError::DbError(DbError::Unavailable { alive, .. }, _) => *alive,
                RequestAttemptError::DbError(DbError::ReadTimeout { received, .. }, _) => *received,
                RequestAttemptError::DbError(DbError::WriteTimeout { received, .. }, _) => {
                    *received
                }
                RequestAttemptError::DbError(DbError::ReadFailure { received, .. }, _) => *received,
                RequestAttemptError::DbError(DbError::WriteFailure { received, .. }, _) => {
                    *received
                }
                _ => -1,
            }
        }
        _ => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_error_result_responses_required(
    error_result: CassBorrowedSharedPtr<CassErrorResult, CConst>,
) -> cass_int32_t {
    let Some(error_result) = ArcFFI::as_ref(error_result) else {
        tracing::error!(
            "Provided null error result pointer to cass_error_result_responses_required!"
        );
        return -1;
    };

    match error_result {
        CassErrorResult::Execution(ExecutionError::LastAttemptError(attempt_error)) => {
            match attempt_error {
                RequestAttemptError::DbError(DbError::Unavailable { required, .. }, _) => *required,
                RequestAttemptError::DbError(DbError::ReadTimeout { required, .. }, _) => *required,
                RequestAttemptError::DbError(DbError::WriteTimeout { required, .. }, _) => {
                    *required
                }
                RequestAttemptError::DbError(DbError::ReadFailure { required, .. }, _) => *required,
                RequestAttemptError::DbError(DbError::WriteFailure { required, .. }, _) => {
                    *required
                }
                _ => -1,
            }
        }
        _ => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_error_result_num_failures(
    error_result: CassBorrowedSharedPtr<CassErrorResult, CConst>,
) -> cass_int32_t {
    let Some(error_result) = ArcFFI::as_ref(error_result) else {
        tracing::error!("Provided null error result pointer to cass_error_result_num_failures!");
        return -1;
    };

    match error_result {
        CassErrorResult::Execution(ExecutionError::LastAttemptError(
            RequestAttemptError::DbError(DbError::ReadFailure { numfailures, .. }, _),
        )) => *numfailures,
        CassErrorResult::Execution(ExecutionError::LastAttemptError(
            RequestAttemptError::DbError(DbError::WriteFailure { numfailures, .. }, _),
        )) => *numfailures,
        _ => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_error_result_data_present(
    error_result: CassBorrowedSharedPtr<CassErrorResult, CConst>,
) -> cass_bool_t {
    let Some(error_result) = ArcFFI::as_ref(error_result) else {
        tracing::error!("Provided null error result pointer to cass_error_result_data_present!");
        return cass_false;
    };

    match error_result {
        CassErrorResult::Execution(ExecutionError::LastAttemptError(
            RequestAttemptError::DbError(DbError::ReadTimeout { data_present, .. }, _),
        )) => {
            if *data_present {
                cass_true
            } else {
                cass_false
            }
        }
        CassErrorResult::Execution(ExecutionError::LastAttemptError(
            RequestAttemptError::DbError(DbError::ReadFailure { data_present, .. }, _),
        )) => {
            if *data_present {
                cass_true
            } else {
                cass_false
            }
        }
        _ => cass_false,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_error_result_write_type(
    error_result: CassBorrowedSharedPtr<CassErrorResult, CConst>,
) -> CassWriteType {
    let Some(error_result) = ArcFFI::as_ref(error_result) else {
        tracing::error!("Provided null error result pointer to cass_error_result_write_type!");
        return CassWriteType::CASS_WRITE_TYPE_UNKNOWN;
    };

    match error_result {
        CassErrorResult::Execution(ExecutionError::LastAttemptError(
            RequestAttemptError::DbError(DbError::WriteTimeout { write_type, .. }, _),
        )) => CassWriteType::from(write_type),
        CassErrorResult::Execution(ExecutionError::LastAttemptError(
            RequestAttemptError::DbError(DbError::WriteFailure { write_type, .. }, _),
        )) => CassWriteType::from(write_type),
        _ => CassWriteType::CASS_WRITE_TYPE_UNKNOWN,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_error_result_keyspace(
    error_result: CassBorrowedSharedPtr<CassErrorResult, CConst>,
    c_keyspace: *mut *const ::std::os::raw::c_char,
    c_keyspace_len: *mut size_t,
) -> CassError {
    let Some(error_result) = ArcFFI::as_ref(error_result) else {
        tracing::error!("Provided null error result pointer to cass_error_result_keyspace!");
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    match error_result {
        CassErrorResult::Execution(ExecutionError::LastAttemptError(
            RequestAttemptError::DbError(DbError::AlreadyExists { keyspace, .. }, _),
        )) => {
            unsafe { write_str_to_c(keyspace.as_str(), c_keyspace, c_keyspace_len) };
            CassError::CASS_OK
        }
        CassErrorResult::Execution(ExecutionError::LastAttemptError(
            RequestAttemptError::DbError(DbError::FunctionFailure { keyspace, .. }, _),
        )) => {
            unsafe { write_str_to_c(keyspace.as_str(), c_keyspace, c_keyspace_len) };
            CassError::CASS_OK
        }
        _ => CassError::CASS_ERROR_LIB_INVALID_ERROR_RESULT_TYPE,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_error_result_table(
    error_result: CassBorrowedSharedPtr<CassErrorResult, CConst>,
    c_table: *mut *const ::std::os::raw::c_char,
    c_table_len: *mut size_t,
) -> CassError {
    let Some(error_result) = ArcFFI::as_ref(error_result) else {
        tracing::error!("Provided null error result pointer to cass_error_result_table!");
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    match error_result {
        CassErrorResult::Execution(ExecutionError::LastAttemptError(
            RequestAttemptError::DbError(DbError::AlreadyExists { table, .. }, _),
        )) => {
            unsafe { write_str_to_c(table.as_str(), c_table, c_table_len) };
            CassError::CASS_OK
        }
        _ => CassError::CASS_ERROR_LIB_INVALID_ERROR_RESULT_TYPE,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_error_result_function(
    error_result: CassBorrowedSharedPtr<CassErrorResult, CConst>,
    c_function: *mut *const ::std::os::raw::c_char,
    c_function_len: *mut size_t,
) -> CassError {
    let Some(error_result) = ArcFFI::as_ref(error_result) else {
        tracing::error!("Provided null error result pointer to cass_error_result_function!");
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    match error_result {
        CassErrorResult::Execution(ExecutionError::LastAttemptError(
            RequestAttemptError::DbError(DbError::FunctionFailure { function, .. }, _),
        )) => {
            unsafe { write_str_to_c(function.as_str(), c_function, c_function_len) };
            CassError::CASS_OK
        }
        _ => CassError::CASS_ERROR_LIB_INVALID_ERROR_RESULT_TYPE,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_error_num_arg_types(
    error_result: CassBorrowedSharedPtr<CassErrorResult, CConst>,
) -> size_t {
    let Some(error_result) = ArcFFI::as_ref(error_result) else {
        tracing::error!("Provided null error result pointer to cass_error_num_arg_types!");
        return 0;
    };

    match error_result {
        CassErrorResult::Execution(ExecutionError::LastAttemptError(
            RequestAttemptError::DbError(DbError::FunctionFailure { arg_types, .. }, _),
        )) => arg_types.len() as size_t,
        _ => 0,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_error_result_arg_type(
    error_result: CassBorrowedSharedPtr<CassErrorResult, CConst>,
    index: size_t,
    arg_type: *mut *const ::std::os::raw::c_char,
    arg_type_length: *mut size_t,
) -> CassError {
    let Some(error_result) = ArcFFI::as_ref(error_result) else {
        tracing::error!("Provided null error result pointer to cass_error_result_arg_type!");
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    match error_result {
        CassErrorResult::Execution(ExecutionError::LastAttemptError(
            RequestAttemptError::DbError(DbError::FunctionFailure { arg_types, .. }, _),
        )) => {
            if index >= arg_types.len() as size_t {
                return CassError::CASS_ERROR_LIB_INDEX_OUT_OF_BOUNDS;
            }
            unsafe {
                write_str_to_c(
                    arg_types[index as usize].as_str(),
                    arg_type,
                    arg_type_length,
                )
            };
            CassError::CASS_OK
        }
        _ => CassError::CASS_ERROR_LIB_INVALID_ERROR_RESULT_TYPE,
    }
}
