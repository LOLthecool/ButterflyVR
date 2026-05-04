extends VBoxContainer
class_name InstancePage

const OBJECT_INFO_ENDPOINT:String = "/api/v0/%s/%s"

@export var details_name:Label
@export var details_description:Label
@export var details_creation_time:Label
@export var details_update_time:Label
@export var details_tags:tags_list
@export var instances_list:InstanceList
@export var instance_creator:InstanceCreator

func show_details(short_world:Dictionary) -> void:
	var response:Array[Variant] = await GlobalAPIHandler.make_request(
			HTTPClient.METHOD_GET, OBJECT_INFO_ENDPOINT % ["World", short_world["id"]], 
			PackedStringArray([GlobalAccountHandler.get_token_header()]))
	@warning_ignore("unsafe_call_argument")
	var result:Array[Variant] = GlobalAPIHandler.handle_response(response[0], response[2], [200], 
			["id", "name", "description", "flags", "updated_at", "created_at", "object_size", "creator", "publicity", "tags"])
	
	if !result[0]:
		push_error("error when getting world info")
		if result[1] != -1:
			push_error("server response: %s" % result[1])
		if result[2] != "":
			push_error("error code: %s" % result[2])
		if result[3] != "":
			push_error("error message: %s" % result[3])
		return
	
	visible = true
	
	var world:Dictionary[String, Variant] = result[4]
	
	@warning_ignore("unsafe_cast")
	instance_creator.world = UUID.from_String(world["id"] as String)
	
	details_name.text = world["name"]
	details_description.text = world["description"]
	details_creation_time.text = str(world["created_at"])
	details_update_time.text = str(world["updated_at"])
	# todo: load world image here
	
	details_tags.show_world_tags(world)
	
	instances_list.show_instances(world)
