extends Button

@export var details_page: AvatarDetailsPage
@export var license_button:LicenseViewButton


func _pressed() -> void:
	details_page.visible = false
	license_button.hide_panel()
