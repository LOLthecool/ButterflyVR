extends Node
class_name MiscHelpers

static func await_lock_mutex(mutex:Mutex) -> void:
	while mutex.try_lock() == false:
		await GlobalTreeAccessHelper.physics_frame
