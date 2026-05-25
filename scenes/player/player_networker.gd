extends NetworkedNode
class_name PlayerNetworker

# todo: get max latency from netnodes, maintain buffer of previous data (pos, rot, vel, inputs) for that long,
# if server sends a position that cant be matched within threshold, change closest match to server value and resimulate
# resim uses deltas between buffer items + inputs to estimate the correct position
# should be able to move_and_slide/collide for collision detection? otherwise might need to get fancy
# if multiple desyncs in a row: snap to server position
# some of this logic probably going in player_desktop

@export var target:Player

func _ready() -> void:
	if get_parent():
		if get_parent().has_meta("owner_id"):
			owner_id = get_parent().get_meta("owner_id", PackedByteArray())
	if owner_id == (await GlobalAccountHandler.get_uuid()).backing_storage and !NetworkManager.is_server():
		target.init_local()
	else:
		target.init_remote()

func _get_networked_values() -> Array:
	var values:Array = []
	values.push_back(target.position)
	values.push_back(target.rotation)
	values.push_back(target.velocity)
	#values.push_back(target.head_ik_target.position)
	#values.push_back(target.head_ik_target.rotation)
	#values.push_back(target.left_arm_ik_target.position)
	#values.push_back(target.left_arm_ik_target.rotation)
	#values.push_back(target.right_arm_ik_target.position)
	#values.push_back(target.right_arm_ik_target.rotation)
	#values.push_back(target.interactor_origin.position)
	#values.push_back(target.interactor_origin.rotation)
	return values

func _set_networked_values(values: Array) -> void:
	target.server_position = values[0]
	target.server_rotation = values[1]
	target.server_velocity = values[2]
	#target.head_ik_target.position = values[3]
	#target.head_ik_target.rotation = values[4]
	#target.left_arm_ik_target.position = values[5]
	#target.left_arm_ik_target.rotation = values[6]
	#target.right_arm_ik_target.position = values[7]
	#target.right_arm_ik_target.rotation = values[8]
	#target.interactor_origin.position = values[9]
	#target.interactor_origin.rotation = values[10]

func _get_networked_value_type(idx: int) -> TypeHelper.NetworkedValueTypes:
	match idx:
		0:
			return TypeHelper.NetworkedValueTypes.Vector3
		1:
			return TypeHelper.NetworkedValueTypes.Vector3
		2:
			return TypeHelper.NetworkedValueTypes.Vector3
		#3:
			#return TypeHelper.NetworkedValueTypes.Vector3
		#4:
			#return TypeHelper.NetworkedValueTypes.Vector3
		#5:
			#return TypeHelper.NetworkedValueTypes.Vector3
		#6:
			#return TypeHelper.NetworkedValueTypes.Vector3
		#7:
			#return TypeHelper.NetworkedValueTypes.Vector3
		#8:
			#return TypeHelper.NetworkedValueTypes.Vector3
		#9:
			#return TypeHelper.NetworkedValueTypes.Vector3
		#10:
			#return TypeHelper.NetworkedValueTypes.Vector3
	return TypeHelper.NetworkedValueTypes.End

func _get_server_priority(_clientid: PackedByteArray) -> int:
	return 1000

func _get_client_priority() -> int:
	return 1000

func _on_owner_dc() -> void:
	target.queue_free()
