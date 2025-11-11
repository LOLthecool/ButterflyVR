extends Node
class_name WorldHandler

const USER_INFO_ENDPOINT:String = "api/v0/user/{0}"
const WORLD_INFO_ENDPOINT:String = "api/v0/world/{0}"

var current_world:WorldController

func load_homeworld() -> void:
	var response:Array[Variant] = await GlobalAPIHandler.make_request(HTTPClient.METHOD_GET, USER_INFO_ENDPOINT % await GlobalAccountHandler.get_uuid(), PackedStringArray([GlobalAccountHandler.get_token_header()]))
	var result = GlobalAPIHandler.handle_response(response[0], response[2], [200], ["homeworld_uuid"])
	var values = result[4]
	if values.is_empty():
		push_error("failed to aquire homeworld")
		if result[2] != -1:
			push_error("error code: %s" % result[2])
		if result[3] != "":
			push_error("error message: %s" % result[3])
		# todo: dump in some kinda of fallback world
		push_error("no error handling here, exiting")
		get_tree().free() # this is fine since we should disconnect before this point
	# get default instance type, if offline load_world(uuid, null), 
	# otherwise create instance then load
	load_world(values[0])

func load_world(world_id:UUID, instance_id:UUID = null) -> void:
	var response:Array[Variant] = await GlobalAPIHandler.make_request(HTTPClient.METHOD_GET, WORLD_INFO_ENDPOINT % await GlobalAccountHandler.get_uuid(), PackedStringArray([GlobalAccountHandler.get_token_header()]))
	var result = GlobalAPIHandler.handle_response(response[0], response[2], [200], [])
	var values = result[4]
	var world:PackedScene = await GlobalDownloadHandler.get_object(world_id, LRUCache.ObjectType.world)
	if !SetupHelpers.check_safe(world.get_state()):
		push_error("tried to load unsafe world, aborting")
		push_error("no error handling here, exiting")
		get_tree().free() # this is fine since we should disconnect before this point
		return
	get_tree().current_scene.queue_free()
	await get_tree().physics_frame
	var root:Node = world.instantiate()
	SetupHelpers.setup_world(root)
	get_tree().root.add_child(root) 
	# todo:
	# if instance id != null call instance handler with instance id
	# else call with null for new offline instance

func disconnect_from_world(go_home:bool = true) -> void:
	NetworkManager.stop()
	if go_home:
		load_homeworld()
