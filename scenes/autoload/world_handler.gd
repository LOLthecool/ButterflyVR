extends Node
class_name WorldHandler

const USER_INFO_ENDPOINT:String = "/api/v0/user/%s"
const WORLD_INFO_ENDPOINT:String = "/api/v0/world/%s"

var current_world:WorldController

func load_homeworld() -> void:
	var response:Array[Variant] = await GlobalAPIHandler.make_request(
			HTTPClient.METHOD_GET, 
			USER_INFO_ENDPOINT % await GlobalAccountHandler.get_uuid(), 
			PackedStringArray([GlobalAccountHandler.get_token_header()]))
	@warning_ignore("unsafe_call_argument")
	var result:Array[Variant] = GlobalAPIHandler.handle_response(response[0], response[2], [200], ["homeworld"])
	var values:Dictionary[String, Variant] = result[4]
	
	if !result[0]:
		push_error("failed to aquire homeworld")
		if result[1] != -1:
			push_error("server response: %s" % result[1])
		if result[2] != "":
			push_error("error code: %s" % result[2])
		if result[3] != "":
			push_error("error message: %s" % result[3])
		await load_fallback_world()
		return
	
	# todo: get default instance type, if offline load_world(uuid), 
	# otherwise create instance then load
	@warning_ignore("unsafe_cast")
	await load_world(UUID.from_String(values["homeworld"] as String))

func load_fallback_world() -> void:
	if get_tree().current_scene:
		get_tree().current_scene.queue_free()
	
	await get_tree().physics_frame
	
	get_tree().change_scene_to_file("res://scenes/world/fallback world/debug_world.tscn")

func load_world(world_id:UUID, instance_id:UUID = null) -> void:
	var world:PackedScene = await GlobalDownloadHandler.get_object(world_id, LRUCache.ObjectType.world)
	
	if world == null:
		await load_fallback_world()
		return
	
	if !SetupHelpers.check_safe(world.get_state()):
		push_error("tried to load unsafe world, aborting")
		load_fallback_world()
		return
	
	get_tree().current_scene.queue_free()
	
	await get_tree().physics_frame
	
	await disconnect_from_world(false)
	
	# initialization order MUST be:
	# 1. world instantaited and setup (to connect signals in _init())
	# 2. instance joined
	# 3. world added to scene tree (to register with client in _ready())
	var root:Node = SetupHelpers.setup_world(world.instantiate())
	
	await get_tree().physics_frame
	
	NetworkManager.kill()
	
	if instance_id:
		if !await GlobalInstanceHandler.join_instance(instance_id):
			load_fallback_world()
			return
	else:
		await GlobalInstanceHandler.create_and_join_offline_instance(world_id)
	
	# client must be started by this point
	get_tree().root.add_child(root) 
	get_tree().current_scene = root
	current_world = root

# server must be started before this is called
func load_world_server(world_id:UUID, bind_port:int) -> void:
	var world:PackedScene = await GlobalDownloadHandler.get_object(world_id, LRUCache.ObjectType.world)
	
	if world == null:
		push_error("failed to load world")
		push_error("no error handling here, exiting")
		get_tree().quit() # this is fine since we should disconnect before this point
		return
	
	if !SetupHelpers.check_safe(world.get_state()):
		push_error("tried to load unsafe world, aborting")
		push_error("no error handling here, exiting")
		get_tree().quit() # this is fine since we should disconnect before this point
		return
	
	var root:Node = SetupHelpers.setup_world(world.instantiate())
	
	# todo: this definetly shouldnt be here 
	# but we need this to happen after the world's _init but before it's _ready
	NetworkManager.start_server(bind_port)
	
	get_tree().root.add_child(root) 
	current_world = root

func disconnect_from_world(go_home:bool = true) -> void:
	NetworkManager.stop()
	await get_tree().physics_frame
	await get_tree().physics_frame
	NetworkManager.kill()
	await get_tree().physics_frame
	if go_home:
		load_homeworld()
