extends Node
class_name PersistanceHandler
# provides a homogenous ConfigFile style interface over multiple files containing persistant data

const CONFIG_PATH:String = "user://config/"
const CONFIG_FILE_EXTENSION:String = ".cfg"

var config_files:Dictionary[String, ConfigFile]

func _init() -> void:
	if !DirAccess.dir_exists_absolute(CONFIG_PATH):
		DirAccess.make_dir_recursive_absolute(CONFIG_PATH)
	for file:String in DirAccess.get_files_at(CONFIG_PATH):
		var new_file:ConfigFile = ConfigFile.new()
		var path:String = CONFIG_PATH + file
		new_file.load(path)
		config_files[path] = new_file

# gets a config value from the specified file, creating it with the default value if it dosent exist
func register_value(file_name:String, catagory:String, key:String, default_value:Variant) -> Variant:
	var file:String = CONFIG_PATH + file_name + CONFIG_FILE_EXTENSION
	
	if config_files.has(file):
		if config_files[file].has_section_key(catagory, key):
			# didnt touch files so we just return directly here
			return config_files[file].get_value(catagory, key)
		else:
			config_files[file].set_value(catagory, key, default_value)
	else:
		var new_file:ConfigFile = ConfigFile.new()
		new_file.set_value(catagory, key, default_value)
		config_files[file] = new_file
	
	config_files[file].save(file)
	return default_value

# falible version of register_value, requires that the specified value exists
func get_value(file_name:String, catagory:String, key:String) -> Variant:
	var file:String = CONFIG_PATH + file_name + CONFIG_FILE_EXTENSION
	return config_files[file].get_value(catagory, key)

# changes the value of an existing key
# errors if the specified key does not exist (create with register_value)
func set_value(file_name:String, catagory:String, key:String, value:Variant) -> void:
	var file:String = CONFIG_PATH + file_name + CONFIG_FILE_EXTENSION
	config_files[file].set_value(catagory, key, value)
	config_files[file].save(file)
