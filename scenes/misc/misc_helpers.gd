extends Node
class_name MiscHelpers


static func await_lock_mutex(mutex: Mutex) -> void:
	while mutex.try_lock() == false:
		await GlobalTreeAccessHelper.physics_frame


static func log_request_error(
	base_message: String,
	response_code: int,
	error_code: String,
	error_message: String,
) -> void:
	push_error(base_message)

	if response_code == -1:
		push_error("server did not respond")
		return

	push_error("server response: %s" % response_code)
	if error_code != "":
		push_error("error code: %s" % error_code)
	if error_message != "":
		push_error("error message: %s" % error_message)
