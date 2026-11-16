-- AgentGrid Setup for macOS: asks for a name, runs install.sh from the app's Resources, shows the result.
on run
	set hub to "https://agentgrid.tminos.com"
	set zh to ((user locale of (system info)) starts with "zh")
	set appTitle to "AgentGrid Setup"

	if (do shell script "uname -m") is not "arm64" then
		if zh then
			set msg to "这台 Mac 是 Intel 芯片，暂时还不支持。"
		else
			set msg to "This Mac has an Intel processor, which is not supported yet."
		end if
		display dialog msg buttons {"OK"} default button 1 with icon stop with title appTitle
		return
	end if

	set defaultName to do shell script "scutil --get ComputerName"
	if zh then
		set askText to "给这台电脑起个名字："
		set okButton to "安装"
		set cancelButton to "取消"
	else
		set askText to "Give this computer a name:"
		set okButton to "Install"
		set cancelButton to "Cancel"
	end if
	set answer to display dialog askText default answer defaultName buttons {cancelButton, okButton} default button okButton cancel button cancelButton with title appTitle
	set nodeName to text returned of answer
	if nodeName is "" then set nodeName to defaultName

	set installer to POSIX path of (path to resource "install.sh")
	try
		do shell script "/bin/bash " & quoted form of installer & " " & quoted form of hub & " " & quoted form of nodeName
		if zh then
			set doneText to "装好了！这台 Mac 已经以「" & nodeName & "」接入。"
		else
			set doneText to "Done! This Mac is now connected as \"" & nodeName & "\"."
		end if
		display dialog doneText buttons {"OK"} default button 1 with title appTitle
	on error errMsg
		if zh then
			set failText to "安装没有完成。请把这个窗口截图发给我们。"
		else
			set failText to "Setup did NOT finish. Please take a screenshot of this window and send it to us."
		end if
		display dialog failText & return & return & errMsg buttons {"OK"} default button 1 with icon stop with title appTitle
	end try
end run
