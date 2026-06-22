extends NetworkedNode
class_name PlayerNetworker

@export var target:Player

func _ready() -> void:
	if owner_id == (await GlobalAccountHandler.get_uuid()).backing_storage and !NetworkManager.is_server():
		target.init_local()
	else:
		target.init_remote()

func _get_networked_values() -> Array:
	var values:Array = []
	values.push_back(target.position)
	values.push_back(target.rotation)
	values.push_back(target.velocity)
	if target.head_ik_target:
		values.push_back(target.head_ik_target.rotation)
	else:
		push_warning("no target")
		values.push_back(Vector3.ZERO)
	return values

func _set_networked_values(values: Array) -> void:
	target.server_position = values[0]
	target.server_rotation = values[1]
	target.server_velocity = values[2]
	if !target.is_local:
		target.head_ik_target.rotation = values[3]

func _get_networked_value_type(idx: int) -> TypeHelper.NetworkedValueTypes:
	match idx:
		0:
			return TypeHelper.NetworkedValueTypes.Vector3
		1:
			return TypeHelper.NetworkedValueTypes.Vector3
		2:
			return TypeHelper.NetworkedValueTypes.Vector3
		3:
			return TypeHelper.NetworkedValueTypes.Vector3
	return TypeHelper.NetworkedValueTypes.End

func _get_server_priority(_clientid: PackedByteArray) -> int:
	return 1000

func _get_client_priority() -> int:
	return 1000

func _on_owner_dc() -> void:
	target.queue_free()
