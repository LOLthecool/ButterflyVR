extends Node
class_name ServerLoader

static func start() -> void:
	print("server starting up")
	
	var key:Array[int] = []
	key.assign(Crypto.new().generate_random_bytes(32))
	
	var bind_port:int = 0
	var api_token:PackedByteArray = PackedByteArray()
	var world:UUID = UUID.new()
	var is_local:bool = false
	
	for argument:String in OS.get_cmdline_args():
		if argument == "--local":
			is_local = true
			continue
		
		var split:PackedStringArray = argument.split("=")
		if split.size() != 2:
			continue
		
		var argument_key:String = split[0].trim_prefix("--")
		var argument_value:String = split[1]
		
		match argument_key:
			"bind_port":
				if argument_value.is_valid_int():
					var port_num:int = int(argument_value)
					if port_num >= 1024 and port_num <= 65535:
						bind_port = port_num
					else:
						push_error("port %s is not a valid port number" % port_num)
				else:
					push_error("expected a port number but got \"%s\"" % argument_value)
			"api_token":
				if argument_value.is_valid_hex_number():
					var token:PackedByteArray = argument_value.hex_decode()
					if token.size() == 64:
						api_token = token
					else:
						push_error("expected token of size 64 but got size %s" % token.size)
				else:
					push_error("expected an api token but got \"%s\"" % argument_value)
			"world":
				var world_uuid:UUID = UUID.from_String(argument_value)
				if world_uuid:
					world = world_uuid
				else:
					push_error("expected a world UUID but got \"%s\"" % argument_value)
	
	GlobalServerHandler.start(api_token, is_local, world, bind_port)
