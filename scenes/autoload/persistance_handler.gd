extends Node
class_name PersistanceHandler
# provides a homogenous ConfigFile style interface over multiple files containing persistant data

const CONFIG_PATH:String = "user://config/%s.cfg"
const CONFIG_DIR:String = "user://config/"

var config_files:Dictionary[String, ConfigFile]

func _init() -> void:
	if !DirAccess.dir_exists_absolute(CONFIG_DIR):
		DirAccess.make_dir_recursive_absolute(CONFIG_DIR)
	for file:String in DirAccess.get_files_at(CONFIG_DIR):
		var new_file:ConfigFile = ConfigFile.new()
		var path:String = CONFIG_DIR + file
		new_file.load(path)
		config_files[path] = new_file

func _exit_tree() -> void:
	flush_all()

# gets a config value from the specified file, creating it with the default value if it dosent exist
func register_value(file_name:String, catagory:String, key:String, default_value:Variant, flush:bool = true) -> Variant:
	var file:String = CONFIG_PATH % file_name
	
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
	
	if flush:
		config_files[file].save(file)
	return default_value

# falible version of register_value, requires that the specified value exists
func get_value(file_name:String, catagory:String, key:String) -> Variant:
	var file:String = CONFIG_PATH % file_name
	return config_files[file].get_value(catagory, key)

func get_catagory(file_name:String, catagory:String) -> Dictionary[String, Variant]:
	var file:String = CONFIG_PATH % file_name
	
	if !config_files.has(file) or !config_files[file].has_section(catagory):
		return {}
	
	var result:Dictionary[String, Variant]
	for key:String in config_files[file].get_section_keys(catagory):
		result[key] = config_files[file].get_value(catagory, key)
	
	return result

func clear_catagory(file_name:String, catagory:String, flush:bool = true) -> void:
	var file:String = CONFIG_PATH % file_name
	
	if !config_files.has(file) or !config_files[file].has_section(catagory):
		return
	
	config_files[file].erase_section(catagory)
	
	if flush:
		config_files[file].save(file)

# changes the value of an existing key
# errors if the specified key does not exist (create with register_value)
func set_value(file_name:String, catagory:String, key:String, value:Variant, flush:bool = true) -> void:
	var file:String = CONFIG_PATH % file_name
	config_files[file].set_value(catagory, key, value)
	
	if flush:
		config_files[file].save(file)

# combined register + set value, sets the key to the value and creates it if it dosent exist
func save_value(file_name:String, catagory:String, key:String, value:Variant, flush:bool = true) -> void:
	if register_value(file_name, catagory, key, value, false) != value:
		set_value(file_name, catagory, key, value, flush)

func flush_file(file_name:String) -> void:
	var file:String = CONFIG_PATH % file_name
	config_files[file].save(file)

func flush_all() -> void:
	for file:String in config_files.keys():
		config_files[file].save(file)
