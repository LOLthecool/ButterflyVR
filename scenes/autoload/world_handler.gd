extends Node
class_name WorldHandler

const HOMEWORLD_REQUEST_ENDPOINT:String = "api/v0/users/{0}/home"
const WORLD_INFO_ENDPOINT:String = "api/v0/world/{0}"

var current_world:WorldController

func load_homeworld() -> void:
	var response:Array[Variant] = await GlobalAPIHandler.make_request(HTTPClient.METHOD_GET, HOMEWORLD_REQUEST_ENDPOINT % await GlobalAccountHandler.get_uuid(), PackedStringArray([GlobalAccountHandler.get_token_header()]))
	var result = GlobalAPIHandler.handle_response(response[0], response[2], [200], ["world_uuid"])
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
	load_world(values[0])

func load_world(world_id:UUID, instance_id:UUID = null) -> void:
	pass
	# todo:
	# get world info
	# verify we can access world
	# load world pack
	# call setup / validate for world pack
	# change scene to world pack
	# if instance id != null call instance handler with instance id
	# else create new offline instance

func disconnect_from_world(go_home:bool = true) -> void:
	pass
