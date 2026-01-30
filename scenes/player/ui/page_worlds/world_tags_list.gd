extends HFlowContainer
class_name tags_list

func show_world_tags(world:Dictionary) -> void:
	for child:Node in get_children():
		child.queue_free()
	
	for tag:String in world["tags"]:
		var tag_button:Button = Button.new()
		tag_button.text = tag
		add_child(tag_button)
