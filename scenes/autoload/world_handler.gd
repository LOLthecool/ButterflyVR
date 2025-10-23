extends Node
class_name WorldHandler

const HOMEWORLD_REQUEST_ENDPOINT:String = "api/v0/users/{0}/home"

var current_world:WorldController

func load_homeworld() -> void:
	var response:Array[Variant] = await GlobalAPIHandler.make_request(HTTPClient.METHOD_GET, HOMEWORLD_REQUEST_ENDPOINT.format(await GlobalAccountHandler.get_uuid()))
	

func load_world(world_id:UUID) -> void:
	pass

func disconnect_from_world(go_home:bool = true) -> void:
	pass
