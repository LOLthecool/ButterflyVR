extends HFlowContainer
class_name FlagList

# all moderation enforced flags, auto flags are automatically applied based on object properties
enum Flags{
	EXPLICIT,
	SUGGESTIVE,
	GORE,
	JUMPSCARE,
	AUTO_VERYBIG,
	AUTO_VERYSMALL,
	AUTO_LONGRANGEEFFECTS,
	AUTO_LOUD,
	AUTO_SCREENSPACE,
}

# strings for flags
const EXPLICIT_MSG:String = "Explicit Content"
const SUGGESTIVE_MSG:String = "Suggestive Content"
const GORE_MSG:String = "Gore/Shock Content"
const JUMPSCARE_MSG:String = "Jumpscare"
const AUTO_VERYBIG_MSG:String = "Very Large"
const AUTO_VERYSMALL_MSG:String = "Very Small"
const AUTO_LONGRANGEEFFECTS_MSG:String = "Long Range Audio / Visuals"
const AUTO_LOUD_MSG:String = "Loud Audio"
const AUTO_SCREENSPACE_MSG:String = "Screenspace Shader"

func create_list(flags:Array) -> void:
	return
	
	# none of this is correct, need to redo when flags is implemented probably
	for child:Node in get_children():
		child.queue_free()
	
	for idx:int in flags:
		var flag:Flags = idx as Flags
		
		var flag_string:String
		match flag:
			Flags.EXPLICIT:
				flag_string = EXPLICIT_MSG
			Flags.SUGGESTIVE:
				flag_string = SUGGESTIVE_MSG
			Flags.GORE:
				flag_string = GORE_MSG
			Flags.JUMPSCARE:
				flag_string = JUMPSCARE_MSG
			
			Flags.AUTO_VERYBIG:
				flag_string = AUTO_VERYBIG_MSG
			Flags.AUTO_VERYSMALL:
				flag_string = AUTO_VERYSMALL_MSG
			Flags.AUTO_LONGRANGEEFFECTS:
				flag_string = AUTO_LONGRANGEEFFECTS_MSG
			Flags.AUTO_LOUD:
				flag_string = AUTO_LOUD_MSG
			Flags.AUTO_SCREENSPACE:
				flag_string = AUTO_SCREENSPACE_MSG
		
		var flag_text:Label = Label.new()
		flag_text.text = flag_string
		flag_text.add_theme_font_size_override("font_size", 24)
		
		var flag_container:PanelContainer = PanelContainer.new()
		flag_container.add_child(flag_text)
		
		add_child(flag_container)
