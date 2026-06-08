extends Node
class_name StringifyHelper

static func stringify_object_publicity(publicity:int) -> String:
	match publicity:
		0:
			return "Private"
		1:
			return "Friends"
		2:
			return "Unlisted"
		3:
			return "Public"
		_:
			return "UNNAMED"

static func stringify_instance_publicity(publicity:int) -> String:
	match publicity:
		0:
			return "Invite Only"
		1:
			return "Friends"
		2:
			return "Friends Of Friends"
		3:
			return "Public"
		_:
			return "UNNAMED"
