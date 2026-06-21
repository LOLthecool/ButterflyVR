extends CCKMarker
class_name IKMarker

func setup(values:Dictionary[String, Variant], target:Node, state:SetupHelpers.SetupState) -> void:
	# todo: check validity of values
	if target is not Skeleton3D:
		return
	
	var head_look_controller:LookAtModifier3D = LookAtModifier3D.new()
	var head_target:Node3D = Node3D.new()
	var head_look_at_target:Node3D = Node3D.new()
	
	target.add_child(head_look_controller)
	target.add_child(head_target)
	head_target.add_child(head_look_at_target)
	
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
	head_twist_propogater.set_joint_twist_amount(0, 0, 0.2)
	head_twist_propogater.set_joint_twist_amount(0, 0, 1)
	head_twist_propogater.set_joint_twist_amount(0, 0, 1)
	
	# todo: ik nodes for vr controls
	
	var state_values:Dictionary[String, Node] = {
			"head_target":head_target,
			"head_view": values["head_view"]
			}
	
	state.state["ik_values"] = state_values

func get_name() -> String:
	return "IKMarker"

func supports_multiple_copies() -> bool:
	return false

func is_allowed_on(object_type: LRUCache.ObjectType) -> bool:
	if object_type == LRUCache.ObjectType.avatar:
		return true
	return false
