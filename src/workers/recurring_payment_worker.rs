//! Recurring payment execution worker.
//!
//! Polls for due schedules every `poll_interval` and executes them using the
//! existing bill-payment / transaction flow. Fully idempotent: the
//! (schedule_id, scheduled_at) unique index prevents double-execution even
//! across worker restarts.

use crate::database::recurring_payment_repository::RecurringPaymentRepository;
use crate::database::transaction_repository::TransactionRepository;
use crate::recurring::frequency::{advance_schedule, Frequency};
use crate::recurring::notification;
use chrono::Utc;
use sqlx::t