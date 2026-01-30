extends VBoxContainer
class_name InstancePage

const OBJECT_INFO_ENDPOINT:String = "/api/v0/%s/%s"

@export var details_name:Label
@export var details_description:Label
@export var details_creation_time:Label
@export var details_update_time:Label
@export var details_tags:tags_list
@export var instances_list:InstanceList

func show_details(short_world:Dictionary) -> void:
	var response = await GlobalAPIHandler.make_request(
			HTTPClient.METHOD_GET, OBJECT_INFO_ENDPOINT % ["World", short_world["id"]], 
			PackedStringArray([GlobalAccountHandler.get_token_header()]))
	var result = await GlobalAPIHandler.handle_response(response[0], response[2], [200], 
			["id", "name", "description", "flags", "updated_at", "created_at", "object_size", "creator", "publicity"])
	
	if !result[0]:
		push_error("error when getting world info")
		return
	
	visible = true
	
	var world = result[4]
	
	details_name = world["name"]
	details_description = world["description"]
	details_creation_time = world["created_at"]
	details_update_time = world["updated_at"]
	# todo: load world image here
	
	details_tags.show_world_tags(world)
	
	instances_list.show_instances(world)
