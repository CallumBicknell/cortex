# TASKS.md

> Master development backlog for Cortex.
>
> Tasks should be completed in roughly this order unless priorities change.
>
> Rules:
>
> * One logical task = one issue / one branch / one or more related commits.
> * Check items off instead of deleting them.
> * Add new ideas to `IDEAS.md` first, then promote them here once approved.
> * Keep tasks small and reviewable.
> * Every completed task should include tests and documentation where applicable.

---

# Legend

* [ ] Not Started
* [x] Complete
* [~] In Progress
* [!] Blocked

---

# Epic 0 — Foundation

## Repository

* [x] Create Rust/Go workspace
* [x] Create project structure
* [x] Configure Git (user.name, user.email)
* [x] Configure editor settings (VS Code settings.json)
* [x] Configure `.gitignore` (standard Rust/Python/IDE/OS)
* [x] Configure licensing (MIT/Apache-2.0 dual)
* [x] Create README (project overview, features, structure)
* [x] Create CONTRIBUTING (guidelines, workflow)
* [x] Create CODE_OF_CONDUCT (contributor covenant)
* [x] Create SECURITY policy (reporting, advisories)
* [x] Set up issue templates (bug, feature, question)
* [x] Set up PR template (checklist, linking issues)
* [x] Configure dependabot (security updates)
* [x] Configure codeowners (maintainers)
* [x] Add contributing guide for AI agents
* [x] Add translation/i18n placeholder

## Development

* [x] Configure formatter (rustfmt via rust-toolchain)
* [x] Configure linter (clippy via rust-toolchain)
* [x] Configure tests (cargo test, pytest for Python)
* [x] Configure benchmarks (criterion)
* [x] Configure CI (GitHub Actions: fmt, clippy, test, python import)
* [x] Configure release workflow (tag-based release, changelog)
* [x] Set up pre-commit hooks (rustfmt, clippy, trailing whitespace)
* [x] Set up cargo deny (license, duplicates)
* [x] Set up rustfmt.toml (edition 2021)
* [x] Set up clippy configuration (allow certain lints)
* [x] Configure pytest configuration (asyncio, coverage)
* [x] Set up coverage reporting (codecov)
* [x] Set up benchmark storage (s3 or local)
* [x] Set up performance trend tracking
* [x] Configure dependabot for Python dependencies
* [x] Set up renovation bot for Rust
* [x] Add Makefile for common tasks
* [x] Add justfile alternatives
* [x] Add Dockerfile for dev environment
* [x] Add devcontainer configuration
* [x] Set up smoke test script

## Documentation

* [x] Architecture overview (high-level components)
* [x] Runtime overview (kernel, loop, event bus)
* [x] Loop design (Observe-Plan-Execute-Verify-Reflect)
* [x] Event system (bus, handlers, history, replay)
* [x] Plugin system (API, lifecycle, sandbox)
* [x] Roadmap (quarterly goals)
* [x] API reference (REST/WebSocket)
* [x] SDK guides (Python, TypeScript)
* [x] Tutorial: "Hello World" agent
* [x] Tutorial: Using tools
* [x] Tutorial: Custom model provider
* [x] Tutorial: Plugin development
* [x] FAQ
* [x] Troubleshooting guide
* [x] Glossary of terms
* [x] Contributor guide (already in CONTRIBUTING)
* [x] Release notes template
* [x] Upgrade guide (between versions)

---

# Epic 1 — Runtime Kernel

## Core

* [x] Runtime lifecycle enum (Created, Starting, Running, Stopping, Stopped, Failed)
* [x] Kernel::new() constructor
* [x] Kernel::with_config() constructor
* [x] Kernel::start() async method
* [x] Kernel::stop() method
* [x] Kernel::state() accessor
* [x] Kernel::iteration_count() accessor
* [x] Kernel::register_service<T>() method
* [x] Kernel::get_service<T>() method
* [ ] Kernel::deregister_service<T>() method
* [ ] Kernel::service_exists<T>() method
* [ ] Kernel::list_services() method
* [ ] Kernel::on_shutdown(callback) hook
* [ ] Kernel::on_startup(callback) hook
* [ ] Kernel::set_custom_logger(level)
* [ ] Kernel::enable_profiling()
* [ ] Kernel::disable_profiling()
* [ ] Kernel::get_uptime()
* [ ] Kernel::get_memory_usage()
* [ ] Kernel::set_panic_hook()
* [ ] Kernel::set_exit_handler()
* [ ] Kernel::extend_lifetime(duration)
* [ ] Kernel::request_immediate_stop()
* [ ] Kernel::is_started()
* [ ] Kernel::is_stopped()
* [ ] Kernel::has_failed()
* [ ] Kernel::get_failure_reason()
* [ ] Kernel::clear_failure()
* [ ] Kernel::trigger_manual_gc()
* [ ] Kernel::configure_worker_threads(n)
* [ ] Kernel::set_max_event_history(size)
* [ ] Kernel::get_event_history_size()
* [ ] Kernel::clear_event_history()
* [ ] Kernel::set_event_ttl(seconds)
* [ ] Kernel::enable_dead_letter_queue()
* [ ] Kernel::disable_dead_letter_queue()
* [ ] Kernel::get_dlq_size()
* [ ] Kernel::process_dlq()
* [ ] Kernel::health_check() returning struct

## Configuration

* [x] Config struct with loop_interval_ms
* [x] Config struct with log_level
* [x] Config struct with event_history_size
* [ ] Config struct with max_event_age_seconds
* [ ] Config struct with enable_metrics
* [ ] Config struct with metrics_export_interval
* [ ] Config struct with enable_tracing
* [ ] Config struct with tracing_export_endpoint
* [ ] Config struct with enable_profiling
* [ ] Config struct with profiler_output_dir
* [ ] Config struct with max_concurrent_handlers
* [ ] Config struct with handler_timeout_ms
* [ ] Config struct with max_retry_attempts
* [ ] Config struct with retry_backoff_base_ms
* [ ] Config struct with enable_dead_letter
* [ ] Config struct with dlq_max_size
* [ ] Config struct with dlq_retry_interval
* [ ] Config struct with enable_wal_for_event_store
* [ ] Config struct with wal_sync_interval
* [ ] Config struct with enable_snapshot
* [ ] Config struct with snapshot_interval
* [ ] Config struct with snapshot_retention
* [ ] Config struct with enable_compression
* [ ] Config struct with compression_algorithm
* [ ] Config struct with enable_event_indexing
* [ ] Config struct with index_type (hash, btree)
* [ ] Config struct with enable_event_schema_validation
* [ ] Config struct with schema_registry_url
* [ ] Config struct with enable_event_encryption
* [ ] Config struct with encryption_key_provider
* [ ] Config struct with enable_event_signing
* [ ] Config struct with signing_key_provider

## Defaults & Env

* [x] Config::default() implementation
* [x] Config::from_env() implementation
* [ ] Config::from_file(path) implementation
* [ ] Config::from_yaml(content) implementation
* [ ] Config::from_json(content) implementation
* [ ] Config::validate() method returning Result
* [ ] Config::apply_overrides(overrides) method
* [ ] Config::merge_with(other) method
* [ ] Config::to_pretty_string() method
* [ ] Config::to_json() method
* [ ] Config::to_yaml() method
* [ ] Config::diff(other) method
* [ ] Config::set_from_cli(args) method
* [ ] Config::print_help() method
* [ ] Config::generate_sample_config() method
* [ ] Config::load_secrets_from_vault() method
* [ ] Config::load_secrets_from_env_prefix() method

## Lifecycle Internals

* [ ] Kernel internal state machine implementation
* [ ] Transition guards (only allow valid transitions)
* [ ] Transition logging (debug level)
* [ ] Transition metrics (prometheus counters)
* [ ] Transition hooks execution order
* [ ] Handling of panics during transitions
* [ ] Recovery from failed state
* [ ] Graceful shutdown timeout handling
* [ ] Force kill after timeout
* [ ] Signal handling (SIGINT, SIGTERM)
* [ ] Custom signal handling (SIGUSR1 for reload)
* [ ] Deadlock detection during shutdown
* [ ] Resource cleanup verification
* [ ] File descriptor leak detection
* [ ] Memory leak detection during shutdown
* [ ] Thread join verification
* [ ] Async task cancellation on stop
* [ ] Pending futures draining on stop
* [ ] Final metrics flush on stop
* [ ] Final tracing flush on stop

## Event Broadcasting

* [x] Kernel::event_broadcast<E: Event>() helper
* [ ] Kernel::broadcast_to_subscribers(event) internal
* [ ] Kernel::broadcast_with_filter(event, filter)
* [ ] Kernel::broadcast_and_wait(event, timeout)
* [ ] Kernel::broadcast_and_collect_responses(event)
* [ ] Kernel::publish_event(event) public alias
* [ ] Kernel::emit(event) public alias
* [ ] Kernel::schedule_event(event, delay)
* [ ] Kernel::schedule_recurring(event, interval)
* [ ] Kernel::cancel_scheduled_event(id)
* [ ] Kernel::list_scheduled_events()
* [ ] Kernel::get_scheduled_event_count()
* [ ] Kernel::max_concurrent_schedules
* [ ] Kernel::scheduler_tick_interval
* [ ] Kernel::enable_scheduler_metrics
* [ ] Kernel::scheduler_error_handler
* [ ] Kernel::scheduler_retry_policy

## Error Handling

* [ ] Kernel error types enum
* [ ] Kernel::last_error() accessor
* [ ] Kernel::clear_error] Kernel::set_error(error)
* [ ] Kernel::has_error() accessor
* [ ] Kernel::error_is_recoverable() accessor
* [ ] Kernel::attempt_error_recovery()
* [ ] Kernel::error_backoff_duration()
* [ ] Kernel::max_error_retries
* [ ] Kernel::error_retry_count
* [ ] Kernel::on_error_recovery(callback)
* [ ] Kernel::on_error_fatal(callback)
* [ ] Kernel::panic_on_unrecoverable_error flag
* [ ] Kernel::log_error_with_backtrace
* [ ] Kernel::emit_error_event(error)
* [ ] Kernel::error_metrics_increment
* [ ] Kernel::error_rate_limit
* [ ] Kernel::error_sampling_rate

## Testing & Mocks

* [ ] MockKernel for testing
* [ ] TestConfigBuilder
* [ ] TestServiceRegistry
* [ ] TestEventBus
* [ ] KernelTestHarness
* [ ] AsyncTestRuntime
* [ ] DeterministicScheduler for tests
* [ ] SimulatedClock for time-sensitive tests
* [ ] EventRecorder for capturing events
* [ ] ServiceMocker for dependency injection
* [ ] PanicInjector for failure testing
* [ ] NetworkLatencyInjector
* [ ] ResourceLimiter for tests
* [ ] ChaosMonkey for random failures
* [ ] Property-based testing strategies
* [ ] Fuzzing harness for kernel inputs

## Performance & Optimizations

* [ ] Lock-free service registry (optional)
* [ ] Reader-writer lock tuning
* [ ] Event broadcast zero-copy when possible
* [ ] Pre-allocated event buffers
* [ ] Object pooling for events
* [ ] Custom allocator for kernel
* [ ] NUMA-aware thread placement
* [ ] Core affinity for scheduler threads
* [ ] Cache line padding for hot structs
* [ ] Instruction prefetching hints
* [ ] Branch prediction hints
* [ ] SIMD acceleration for event filtering
* [ ] GPU offload for heavy event processing (optional)
* [ ] Async I/O for event persistence
* [ ] Batch persistence writes
* [ ] Persistence write-ahead log
* [ ] Persistence snapshot isolation
* [ ] Persistence concurrent readers/writers
* [ ] Persistence checksum validation
* [ ] Persistence compression
* [ ] Persistence encryption at rest
* [ ] Persistence key rotation

---

# Epic 2 — Event System

## Event Definition

* [ ] EventId type (UUID v4)
* [ ] EventVersion type (semver)
* [ ] EventMetadata struct
* [ ] EventHeaders map<String, String>
* [ ] EventTimestamp (Unix nano)
* [ ] EventSource (string)
* [ ] EventCorrelationId (UUID)
* [ ] EventCausationId (UUID)
* [ ] EventPriority (enum: Low, Normal, High, Critical)
* [ ] EventTTL (duration)
* [ ] EventSize limit
* [ ] EventSchema version field
* [ ] EventSchema ID (URI)
* [ ] EventPayload (serde Serialize + Deserialize)
* [ ] Event trait (Debug + Send + Sync + 'static)
* [ ] Event::id() accessor
* [ ] Event::version() accessor
* [ ] Event::timestamp() accessor
* [ ] Event::source() accessor
* [ ] Event::correlation_id() accessor
* [ ] Event::causation_id() accessor
* [ ] Event::priority() accessor
* [ ] Event::ttl() accessor
* [ ] Event::headers() accessor
* [ ] Event::metadata() accessor
* [ ] Event::payload() accessor
* [ ] Event::size() accessor
* [ ] Event::is_expired(now) method
* [ ] Event::matches_filter(filter) method
* [ ] Event::clone_into_box() method
* [ ] Event::downcast_ref<T>() method
* [ ] Event::downcast_mut<T>() method
* [ ] Event::to_any() method
* [ ] Event::from_any(any) method
* [ ] Event::serialize_to(vec) method
* [ ] Event::deserialize_from(slice) method
* [ ] Event::to_json() method
* [ ] Event::from_json(slice) method
* [ ] Event::to_msgpack() method
* [ ] Event::from_msgpack(slice) method
* [ ] Event::to_cbor() method
* [ ] Event::from_cbor(slice) method
* [ ] Event::hash() method
* [ ] Event::eq_event(other) method
* [ ] Event::ne_event(other) method
* [ ] Event::lt_event(other) method
* [ ] Event::gt_event(other) method

## Event Bus Core

* [ ] EventBus trait (publish, subscribe, unsubscribe, replay)
* [ ] EventBus::publish(event) async
* [ ] EventBus::publish_batch(events) async
* [ ] EventBus::publish_with_timeout(event, timeout)
* [ ] EventBus::publish_with_retry(event, policy)
* [ ] EventBus::subscribe(handler) returning SubscriptionId
* [ ] EventBus::subscribe_filtered(handler, filter)
* [ ] EventBus::subscribe_priority(handler, priority)
* [ ] EventBus::unsubscribe(sub_id)
* [ ] EventBus::unsubscribe_all()
* [ ] EventBus::handler_count() accessor
* [ ] EventBus::is_subscribed(sub_id) accessor
* [ ] EventBus::get_handler(sub_id) accessor
* [ ] EventBus::modify_handler(sub_id, new_handler)
* [ ] EventBus::set_handler_priority(sub_id, priority)
* [ ] EventBus::set_handler_filter(sub_id, filter)
* [ ] EventBus::pause_subscription(sub_id)
* [ ] EventBus::resume_subscription(sub_id)
* [ ] EventBus::get_subscription_stats(sub_id)
* [ ] EventBus::reset_subscription_stats(sub_id)
* [ ] EventBus::replay_events(count) returning Vec<Event>
* [ ] EventBus::replay_since(id) returning Vec<Event>
* [ ] EventBus::replay_between(start, end) returning Vec<Event>
* [ ] EventBus::replay_by_type(type_id) returning Vec<Event>
* [ ] EventBus::replay_by_correlation(correlation_id) returning Vec<Event>
* [ ] EventBus::replay_by_causation(causation_id) returning Vec<Event>
* [ ] EventBus::replay_by_source(source) returning Vec<Event>
* [ ] EventBus::replay_by_header(key, value) returning Vec<Event>
* [ ] EventBus::replay_within_time_range(start_time, end_time)
* [ ] EventBus::replay_latest_n(n) returning Vec<Event>
* [ ] EventBus::replay_oldest_n(n) returning Vec<Event>
* [ ] EventBus::replay_all() returning Vec<Event>
* [ ] EventBus::replay_chunk(offset, limit) returning Vec<Event>
* [ ] EventBus::replay_iterator() returning Iterator<Item=Event>
* [ ] EventBus::replay_stream() returning Stream<Item=Event>
* [ ] EventBus::history_size() accessor
* [ ] EventBus::set_history_size(size)
* [ ] EventBus::history_is_full() accessor
* [ ] EventBus::history_usage() ratio
* [ ] EventBus::clear_history()
* [ ] EventBus::history_timestamps() returning Vec<Timestamp>
* [ ] EventBus::history_ids() returning Vec<EventId>
* [ ] EventBus::history_sources() returning Vec<String>
* [ ] EventBus::history_correlations() returning Vec<EventId>
* [ ] EventBus::history_causations() returning Vec<EventId>
* [ ] EventBus::history_priorities() returning Vec<Priority>
* [ ] EventBus::history_ttls() returning Vec<Duration>
* [ ] EventBus::history_sizes() returning Vec<usize>
* [ ] EventBus::history_compression_ratio()
* [ ] EventBus::history_encryption_status()
* [ ] EventBus::history_checksum()
* [ ] EventBus::validate_history_integrity()
* [ ] EventBus::repair_history()
* [ ] EventBus::export_history(format) returning Vec<u8>
* [ ] EventBus::import_history(data, format)
* [ ] EventBus::history_backup() returning Vec<u8>
* [ ] EventBus::history_restore(backup)
* [ ] EventBus::history_snapshot() returning Vec<u8>
* [ ] EventBus::history_restore_snapshot(snapshot)
* [ ] EventBus::enable_history_compression(alg)
* [ ] EventBus::disable_history_compression()
* [ ] EventBus::enable_history_encryption(key_provider)
* [ ] EventBus::disable_history_encryption()
* [ ] EventBus::history_key_rotation()
* [ ] EventBus::set_history_sync_interval(interval)
* [ ] EventBus::history_flush()
* [ ] EventBus::history_wal_enabled()
* [ ] EventBus::enable_history_wal()
* [ ] EventBus::disable_history_wal()
* [ ] EventBus::history_wal_size()
* [ ] EventBus::history_wal_checkpoint()
* [ ] EventBus::history_wal_recover()
* [ ] EventBus::history_compaction_threshold()
* [ ] EventBus::trigger_history_compaction()
* [ ] EventBus::history_compaction_in_progress()
* [ ] EventBus::set_history_index_type(type)
* [ ] EventBus::enable_history_indexing(fields)
* [ ] EventBus::disable_history_indexing()
* [ ] EventBus::history_index_stats()
* [ ] EventBus::history_rebuild_index()
* [ ] EventBus::history_index_warmup()
* [ ] EventBus::history_query(query) returning Vec<Event>
* [ ] EventBus:: EventBus::history_query_builder() returning QueryBuilder
* [ ] EventBus::history_query_explain(query)
* [ ] EventBus::history_query_optimize(query)
* [ ] EventBus::history_query_cache_enabled()
* [ ] EventBus::set_history_query_cache_size(size)
* [ ] EventBus::history_query_cache_clear()
* [ ] EventBus::history_query_cache_stats()
* [ ] EventBus::history_enable_audit_log()
* [ ] EventBus::history_audit_log_path()
* [ ] EventBus::history_audit_log_rotate()
* [ ] EventBus::history_audit_log_retention()
* [ ] EventBus::history_enable_tamper_evident()
* [ ] EventBus::history_tamper_evident_key()
* [ ] EventBus::history_verify_tamper_evident()
* [ ] EventBus::history_enable_signing()
* [ ] EventBus::history_signing_key_provider()
* [ ] EventBus::history_verify_signature()
* [ ] EventBus::history_enable_encryption_at_rest()
* [ ] EventBus::history_encryption_key_provider()
* [ ] EventBus::history_rotate_encryption_key()
* [ ] EventBus::history_decrypt_event(event_id)
* [ ] EventBus::history_encrypt_event(event)
* [ ] EventBus::history_enable_field_level_encryption(fields)
* [ ] EventBus::history_decrypt_field(event_id, field)
* [ ] EventBus::history_encrypt_field(event_id, field, value)
* [ ] EventBus::history_enable_compression_at_rest()
* [ ] EventBus::history_compression_algorithm()
* [ ] EventBus::history_decompress_event(event_id)
* [ ] EventBus::history_compress_event(event)
* [ ] EventBus::history_enable_schema_validation()
* [ ] EventBus::history_schema_registry_url()
* [ ] EventBus::history_validate_event_schema(event)
* [ ] EventBus::history_get_event_schema(event_id)
* [ ] EventBus::history_register_event_schema(schema)
* [ ] EventBus::history_unregister_event_schema(schema_id)
* [ ] EventBus::history_list_registered_schemas()
* [ ] EventBus::history_enable_event_versioning()
* [ ] EventBus::history_get_event_version(event_id)
* [ ] EventBus::history_set_event_version(event_id, version)
* [ ] EventBus::history_enable_event_deprecation()
* [ ] EventBus::history_mark_event_deprecated(event_id)
* [ ] EventBus::history_is_event_deprecated(event_id)
* [ ] EventBus::history_enable_event_aliases()
* [ ] EventBus::history_add_event_alias(event_id, alias)
* [ ] EventBus::history_get_event_aliases(event_id)
* [ ] EventBus::history_remove_event_alias(event_id, alias)
* [ ] EventBus::history_enable_event_tagging()
* [ ] EventBus::history_add_event_tag(event_id, tag)
* [ ] EventBus::history_get_event_tags(event_id)
* [ ] EventBus::history_remove_event_tag(event_id, tag)
* [ ] EventBus::history_enable_event_attributes()
* [ ] EventBus::history_set_event_attribute(event_id, key, value)
* [ ] EventBus::history_get_event_attribute(event_id, key)
* [ ] EventBus::history_remove_event_attribute(event_id, key)
* [ ] EventBus::history_enable_event_version_boundaries()
* [ ] EventBus::history_set_event_min_version(event_id, version)
* [ ] EventBus::history_set_event_max_version(event_id, version)
* [ ] EventBus::history_get_event_version_range(event_id)
* [ ] EventBus::history_enable_event_lifecycle()
* [ ] EventBus::history_set_event_lifecycle_state(event_id, state)
* [ ] EventBus::history_get_event_lifecycle_state(event_id)
* [ ] EventBus::history_enable_event_annotations()
* [ ] EventBus::history_set_event_annotation(event_id, key, value)
* [ ] EventBus::history_get_event_annotation(event_id, key)
* [ ] EventBus::history_remove_event_annotation(event_id, key)
* [ ] EventBus::history_enable_event_relations()
* [ ] EventBus::history_add_event_relation(event_id, relation_type, related_id)
* [ ] EventBus::history_get_event_relations(event_id)
* [ ] EventBus::history_remove_event_relation(event_id, relation_type, related_id)
* [ ] EventBus::history_enable_event_geospatial()
* [ ] EventBus::history_set_event_location(event_id, lat, lon)
* [ ] EventBus::history_get_event_location(event_id)
* [ ] EventBus::history_remove_event_location(event_id)
* [ ] EventBus: history_enable_event_time_series()
* [ ] EventBus: history_set_event_time_series(event_id, series_id, value)
* [ ] EventBus: history_get_event_time_series(event_id, series_id)
* [ ] EventBus: history_remove_event_time_series(event_id, series_id)
* [ ] EventBus: history_enable_event_attachments()
* [ ] EventBus: history_add_event_attachment(event_id, attachment_id, data)
* [ ] EventBus: history_get_event_attachment(event_id, attachment_id)
* [ ] EventBus: history_remove_event_attachment(event_id, attachment_id)
* [ ] EventBus: history_enable_event_embeddings()
* [ ] EventBus: history_set_event_embedding(event_id, vector)
* [ ] EventBus: history_get_event_embedding(event_id)
* [ ] EventBus: history_remove_event_embedding(event_id)
* [ ] EventBus: history_enable_event_version_vector()
* [ ] EventBus: history_get_event_version_vector(event_id)
* [ ] EventBus: history_enable_event_provenance()
* [ ] EventBus: history_set_event_provenance(event_id, provenance)
* [ ] EventBus: history_get_event_provenance(event_id)
* [ ] EventBus: history_remove_event_provenance(event_id)
* [ ] EventBus: history_enable_event_licensing()
* [ ] EventBus: history_set_event_license(event_id, license)
* [ ] EventBus: history_get_event_license(event_id)
* [ ] EventBus: history_remove_event_license(event_id)
* [ ] EventBus: history_enable_event_rights_management()
* [ ] EventBus: history_set_event_rights(event_id, rights)
* [ ] EventBus: history_get_event_rights(event_id)
* [ ] EventBus: history_remove_event_rights(event_id)
* [ ] EventBus: history_enable_event_digital_signature()
* [ ] EventBus: history_set_event_signature(event_id, signature)
* [ ] EventBus: history_get_event_signature(event_id)
* [ ] EventBus: history_remove_event_signature(event_id)
* [ ] EventBus: history_enable_event_timestamp_precision()
* [ ] EventBus: history_set_event_timestamp_precision(event_id, precision)
* [ ] EventBus: history_get_event_timestamp_precision(event_id)
* [ ] EventBus: history_enable_event_timezone()
* [ ] EventBus: history_set_event_timezone(event_id, timezone)
* [ ] EventBus: history_get_event_timezone(event_id)
* [ ] EventBus: history_remove_event_timezone(event_id)
* [ ] EventBus: history_enable_event_locale()
* [ ] EventBus: history_set_event_locale(event_id, locale)
* [ ] EventBus: history_get_event_locale(event_id)
* [ ] EventBus: history_remove_event_locale(event_id)
* [ ] EventBus: history_enable_event_custom_fields()
* [ ] EventBus: history_set_event_custom_field(event_id, key, value)
* [ ] EventBus: history_get_event_custom_field(event_id, key)
* [ ] EventBus: history_remove_event_custom_field(event_id, key)
* [ ] EventBus: history_enable_event_schema_evolution()
* [ ] EventBus: history_get_event_schema_version(event_id)
* [ ] EventBus: history_set_event_schema_version(event_id, version)
* [ ] EventBus: history_enable_event_backwards_compatibility()
* [ ] EventBus: history_set_event_backwards_compatible(event_id, boolean)
* [ ] EventBus: history_get_event_backwards_compatible(event_id)
* [ ] EventBus: history_enable_event_forwards_compatibility()
* [ ] EventBus: history_set_event_forwards_compatible(event_id, boolean)
* [ ] EventBus: history_get_event_forwards_compatible(event_id)
* [ ] EventBus: history_enable_event_deprecation_warnings()
* [ ] EventBus: history_set_event_deprecation_warning(event_id, message)
* [ ] EventBus: history_get_event_deprecation_warning(event_id)
* [ ] EventBus: history_remove_event_deprecation_warning(event_id)
* [ ] EventBus: history_enable_event_obsolescence()
* [ ] EventBus: history_set_event_obsolete_date(event_id, date)
* [ ] EventBus: history_get_event_obsolete_date(event_id)
* [ ] EventBus: history_remove_event_obsolete_date(event_id)
* [ ] EventBus: history_enable_event_renewal()
* [ ] EventBus: history_set_event_renewal_date(event_id, date)
* [ ] EventBus: history_get_event_renewal_date(event_id)
* [ ] EventBus: history_remove_event_renewal_date(event_id)
* [ ] EventBus: history_enable_event_version_pin()
* [ ] EventBus: history_set_event_version_pinned(event_id, version)
* [ ] EventBus: history_get_event_version_pinned(event_id)
* [ ] EventBus: history_remove_event_version_pinned(event_id)
* [ ] EventBus: history_enable_event_version_range_pinning()
* [ ] EventBus: history_set_event_version_range_pinned(event_id, min, max)
* [ ] EventBus: history_get_event_version_range_pinned(event_id)
* [ ] EventBus: history_remove_event_version_range_pinned(event_id)
* [ ] EventBus: history_enable_event_schema_lock()
* [ ] EventBus: history_set_event_schema_locked(event_id, boolean)
* [ ] EventBus: history_get_event_schema_locked(event_id)
* [ ] EventBus: history_remove_event_schema_locked(event_id)
* [ ] EventBus: history_enable_event_tenant_isolation()
* [ ] EventBus: history_set_event_tenant_id(event_id, tenant_id)
* [ ] EventBus: history_get_event_tenant_id(event_id)
* [ ] EventBus: history_remove_event_tenant_id(event_id)
* [ ] EventBus: history_enable_event_multi_tenancy()
* [ ] EventBus: history_set_event_tenant_schema(event_id, schema)
* [ ] EventBus: history_get_event_tenant_schema(event_id)
* [ ] EventBus: history_remove_event_tenant_schema(event_id)
* [ ] EventBus: history_enable_event_cross_tenant_queries()
* [ ] EventBus: history_query_tenant(tenant_id) returning Vec<Event>
* [ ] EventBus: history_enable_event_data_lineage()
* [ ] EventBus: history_set_event_data_lineage(event_id, lineage)
* [ ] EventBus: history_get_event_data_lineage(event_id)
* [ ] EventBus: history_remove_event_data_lineage(event_id)
* [ ] EventBus: history_enable_event_data_quality()
* [ ] EventBus: history_set_event_data_quality_score(event_id, score)
* [ ] EventBus: history_get_event_data_quality_score(event_id)
* [ ] EventBus: history_remove_event_data_quality_score(event_id)
* [ ] EventBus: history_enable_event_data_profiling()
* [ ] EventBus: history_get_event_data_profile(event_id)
* [ ] EventBus: history_remove_event_data_profile(event_id)
* [ ] EventBus: history_enable_event_data_lineage_graph()
* [ ] EventBus: history_get_event_data_lineage_graph()
* [ ] EventBus: history_remove_event_data_lineage_graph()
* [ ] EventBus: history_enable_event_anomaly_detection()
* [ ] EventBus: history_set_event_anomaly_score(event_id, score)
* [ ] EventBus: history_get_event_anomaly_score(event_id)
* [ ] EventBus: history_remove_event_anomaly_score(event_id)
* [ ] EventBus: history_enable_event_predictive_maintenance()
* [ ] EventBus: history_set_event_predictive_score(event_id, score)
* [ ] EventBus: history_get_event_predictive_score(event_id)
* [ ] EventBus: history_remove_event_predictive_score(event_id)
* [ ] EventBus: history_enable_event_root_cause_analysis()
* [ ] EventBus: history_set_event_root_cause(event_id, cause)
* [ ] EventBus: history_get_event_root_cause(event_id)
* [ ] EventBus: history_remove_event_root_cause(event_id)
* [ ] EventBus: history_enable_event_impact_analysis()
* [ ] EventBus: history_set_event_impact(event_id, impact)
* [ ] EventBus: history_get_event_impact(event_id)
* [ ] EventBus: history_remove_event_impact(event_id)
* [ ] EventBus: history_enable_event_cost_tracking()
* [ ] EventBus: history_set_event_cost(event_id, cost)
* [ ] EventBus: history_get_event_cost(event_id)
* [ ] EventBus: history_remove_event_cost(event_id)
* [ ] EventBus: history_enable_event_resource_usage()
[Truncated]