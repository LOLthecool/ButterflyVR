extends ScrollContainer
class_name InstanceList

const INSTANCE_SEARCH_ENDPOINT:String = "/api/v0/instances/search"
const INSTANCE_JOIN_ENDPOINT:String = "/api/v0/instances/%s/join"

@export var instances_container:VBoxContainer

var world_id:String
var filters:Dictionary

func update_filters(filters:Dictionary) -> void:
	pass

func show_instances(world:Dictionary) -> void:
	for child:Node in instances_container.get_children():
		child.queue_free()
	
	world_id = world["id"]
	filters["world"] = world_id
	
	var body:String = JSON.stringify(filters)
	
	var response:Array[Variant] = await GlobalAPIHandler.make_request(
			HTTPClient.METHOD_POST, 
			INSTANCE_SEARCH_ENDPOINT, 
			PackedStringArray([GlobalAccountHandler.get_token_header()]), 
			body)
	var result:Array[Variant] = GlobalAPIHandler.handle_response(response[0], response[2], [200], ["instances"])
	
	if !result[0]:
		push_error("error when getting instances")
		if result[1] != -1:
			push_error("server response: %s" % result[1])
		if result[2] != "":
			push_error("error code: %s" % result[2])
		if result[3] != "":
			push_error("error message: %s" % result[3])
		return
	
	for instance:Dictionary in result[4]["instances"]:
		var id:UUID = UUID.from_String(instance["id"])
		var instance_name:String = instance["name"]
		var player_count:int = 0 # todo
		var max_players:int = instance["max_players"]
		var publicity:int = instance["publicity"]
		
		var publicity_string:String = ""
		match publicity:
			0:
				publicity_string = "Private"
			1:
				publicity_string = "Unlisted"
			2:
				publicity_string = "Friends"
			3:
				publicity_string = "Public"
			_:
				publicity_string = "INVALID"
		
		var player_count_string:String = "%s/%s" % [player_count, max_players]
		
		var listing:InstanceListing = preload(
				"res://scenes/player/ui/page_worlds/instance_listing.tscn").instantiate()
		
		listing.instance_name.text = instance_name
		listing.publicity.text = publicity_string
		listing.player_count.text = player_count_string
		listing.join_button.pressed.connect(on_join_button_pressed.bind(id))

func on_join_button_pressed(instance:UUID) -> void:
	GlobalWorldHandler.load_world(UUID.from_String(world_id), instance)
