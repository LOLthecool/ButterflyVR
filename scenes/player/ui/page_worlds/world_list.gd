extends HFlowContainer
class_name WorldList

const SEARCH_ENDPOINT:String = "api/v0/search/%s"

@export var instance_page:InstancePage

func get_and_show_worlds(search_string:String, filters:Dictionary[String ,String]) -> void:
	for child:Node in get_children():
		child.queue_free()
	
	filters["is"] = "world"
	
	var filter_string:String = ""
	for key:String in filters.keys():
		filter_string += "%s:%s," % [key, filters[key]]
	
	# remove & to stop users accidentally breaking the filters
	search_string = search_string.remove_char("&".unicode_at(0))
	
	var search:String = "%s&%s" % [search_string, filter_string]
	var response:Array[Variant] = await GlobalAPIHandler.make_request(
			HTTPClient.METHOD_GET, 
			SEARCH_ENDPOINT % search, 
			PackedStringArray([GlobalAccountHandler.get_token_header()]))
	var result:Array[Variant] = GlobalAPIHandler.handle_response(response[0], response[2], [200], ["worlds"])
	if !result[0]:
		var error_msg:Label = Label.new()
		error_msg.text = "error while retriving worlds, please try again"
		add_child(error_msg)
		push_error("error while retriving worlds")
		if result[1] != -1:
			push_error("server response: %s" % result[1])
		if result[2] != "":
			push_error("error code: %s" % result[2])
		if result[3] != "":
			push_error("error message: %s" % result[3])
		return
	
	for world:Dictionary in result[4]:
		var avatar_listing:ObjectListing = ObjectListing.new()
		avatar_listing.create(world, LRUCache.ObjectType.world)
		avatar_listing.object_selected.connect(instance_page.show_details)
		add_child(avatar_listing)
