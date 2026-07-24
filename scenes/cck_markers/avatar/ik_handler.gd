extends CCKMarker
class_name IKMarker

func setup(values:Dictionary[String, Variant], target:Node, state:SetupHelpers.SetupState) -> void:
	if target is not Skeleton3D:
		return
	
	if state.state["is_local"]:
		@warning_ignore("unsafe_cast")
		(target as Skeleton3D).set_bone_pose_scale(values["head_bone"] as int, Vector3(0.001, 0.001, 0.001))
	
	var head_look_controller:LookAtModifier3D = LookAtModifier3D.new()
	var head_target:Node3D = Node3D.new()
	var head_look_at_target:Node3D = Node3D.new()
	
	target.add_child(head_look_controller)
	target.add_child(head_target)
	head_target.add_child(head_look_at_target)
	
	@warning_ignore("unsafe_cast")
	head_target.position = (target as Skeleton3D).get_bone_global_pose(values["head_bone"] as int).origin
	@warning_ignore("unsafe_cast")
	head_target.quaternion = (
			target as Skeleton3D).get_bone_global_pose(
			values["head_bone"] as int).basis.get_rotation_quaternion()
	head_look_at_target.position.z -= 1
	@warning_ignore("unsafe_cast")
	head_look_controller.bone = values["head_bone"] as int
	head_look_controller.target_node = head_look_controller.get_path_to(head_look_at_target)
	
	var head_twist_propogater:BoneTwistDisperser3D = BoneTwistDisperser3D.new()
	target.add_child(head_twist_propogater)
	
	head_twist_propogater.setting_count = 1
	@warning_ignore("unsafe_cast")
	head_twist_propogater.set_root_bone(0, values["chest_bone"] as int)
	@warning_ignore("unsafe_cast")
	head_twist_propogater.set_end_bone(0, values["head_bone"] as int)
	head_twist_propogater.set_extend_end_bone(0, true)
	head_twist_propogater.set_disperse_mode(0, BoneTwistDisperser3D.DISPERSE_MODE_CUSTOM)
	head_twist_propogater.set_joint_twist_amount.call_deferred(0, 0, 0.1)
	head_twist_propogater.set_joint_twist_amount.call_deferred(0, 1, 0.5)
	
	# todo: ik nodes for vr controls
	
	var state_values:Dictionary[String, Variant] = {
			"head_target":head_target,
			"head_view": values["head_view"] - head_target.position
			}
	
	state.state["ik_values"] = state_values

func perform_migrations(values:Dictionary[String, Variant]) -> Dictionary[String, Variant]:
	var current_version:String = get_current_version_string()
	match values["version"]:
		current_version:
			return values
		_:
			push_error("had no valid migration for %s version %s. content may be broken" \
			% [get_name(), values["version"]])
			return values

func get_current_version_string() -> String:
	return "1"

func get_name() -> String:
	return "IKMarker"

func supports_multiple_copies() -> bool:
	return false

func is_allowed_on(object_type: LRUCache.ObjectType) -> bool:
	if object_type == LRUCache.ObjectType.avatar:
		return true
	return false
