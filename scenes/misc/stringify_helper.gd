extends Node
class_name StringifyHelper


static func stringify_object_publicity(publicity: int) -> String:
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


static func stringify_instance_publicity(publicity: int) -> String:
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


static func stringify_size_kb(size: int) -> String:
	const KILOBYTE: int = 1024
	const MEGABYTE: int = KILOBYTE * 1024
	const GIGABYTE: int = MEGABYTE * 1024
	if size > GIGABYTE:
		return "%.2f GB" % ((size as float) / GIGABYTE)
	elif size > MEGABYTE:
		return "%.2f MB" % ((size as float) / MEGABYTE)
	else:
		return "%.2f KB" % ((size as float) / KILOBYTE)
