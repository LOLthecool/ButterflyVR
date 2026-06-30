extends RayCast3D

@onready var grab_target:Node3D = get_child(0)

var grab_handler:PlayerGrabHandler = GlobalWorldHandler.current_world.player_grab_handler
var is_grabbing:bool = false
var wants_to_grab:bool = false
var grabbed_node:Node3D

func _ready() -> void:
	GlobalWorldHandler.current_world.player_grab_handler.player_grabbed.connect(on_confirmed_grab)

func _unhandled_input(event: InputEvent) -> void:
	if event.is_action_pressed("player_grab"):
		wants_to_grab = true
	if event.is_action_released("player_grab"):
		is_grabbing = false
		wants_to_grab = false

func _physics_process(_delta: float) -> void:
	if wants_to_grab:
		wants_to_grab = false
		force_raycast_update()
		if is_colliding():
			var collider:CollisionObject3D = get_collider()
			
			if collider.has_meta("grabbable_info"):
				var collider_info:Dictionary[String, Variant] = collider.get_meta("grabbable_info")
				
				if collider_info["max_grab_distance"] != -1 and \
						collider_info["max_grab_distance"] < \
						absf((get_collision_point() - global_position).length()):
					return
				
				grab_handler.send_message(
						(await GlobalAccountHandler.get_uuid()).backing_storage, 
						collider.get_parent().get_path())
		return
	
	if is_grabbing:
		grabbed_node.global_position = grab_target.global_position
	else:
		if grabbed_node != null:
			if grabbed_node is RigidBody3D:
				(grabbed_node as RigidBody3D).freeze = false
			
			grabbed_node = null
			grab_target.position = Vector3.ZERO
			grab_handler.send_message((await GlobalAccountHandler.get_uuid()).backing_storage, "")
			
			# todo: enabling this seems to cause on_confirmed_grab / on_grab to trigger twice, with the second call failing to get_node despite the path definetly being valid? this is very weird
			#if get_collider().has_method("on_release"):
			#	get_collider().on_release()

func on_confirmed_grab(player:PackedByteArray, target:Node) -> void:
	if !(player == (await GlobalAccountHandler.get_uuid()).backing_storage):
		return
	
	if is_instance_of(target, Node3D):
		is_grabbing = true
		grab_target.global_position = (target as Node3D).global_position
		grabbed_node = target
		if grabbed_node is RigidBody3D:
			(grabbed_node as RigidBody3D).freeze = true
