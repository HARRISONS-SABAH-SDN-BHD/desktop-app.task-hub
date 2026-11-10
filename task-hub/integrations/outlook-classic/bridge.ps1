$ErrorActionPreference='Stop'
[Console]::InputEncoding=[System.Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding=[System.Text.UTF8Encoding]::new($false)
try {
 $request=[Console]::In.ReadToEnd() | ConvertFrom-Json
 if ($request.operation -notin @('connect','folders','messages','attachments','download')) {throw 'Invalid operation'}
 try {$outlook=[Runtime.InteropServices.Marshal]::GetActiveObject('Outlook.Application')}catch{$outlook=New-Object -ComObject Outlook.Application}
 $session=$outlook.GetNamespace('MAPI')
 $result=$null
 if ($request.operation -eq 'connect') {$null=$session.GetDefaultFolder(6);$result=@{connected=$true;profile=[string]$session.CurrentProfileName}}
 elseif ($request.operation -eq 'folders') {
  $list=[System.Collections.Generic.List[object]]::new()
  function Add-Folder($folder,$label,$depth) {
   if ($depth -gt 10 -or $list.Count -ge 500){return}
   $list.Add(@{id=([string]$folder.EntryID);store_id=([string]$folder.StoreID);name=$label})
   foreach($child in $folder.Folders){Add-Folder $child ($label+' / '+$child.Name) ($depth+1)}
  }
  foreach($store in $session.Stores){$root=$store.GetRootFolder();Add-Folder $root ([string]$store.DisplayName) 0}
  $result=@($list.ToArray())
 }
 elseif ($request.operation -eq 'messages') {
  $folder=$session.GetFolderFromID([string]$request.folder_id,[string]$request.store_id)
  $items=$folder.Items;$items.Sort('[ReceivedTime]',$true)
  $list=[System.Collections.Generic.List[object]]::new();$seen=0
  foreach($item in $items){$seen++;if($seen -gt 500 -or $list.Count -ge 50){break};if($item.Class -ne 43){continue}
   $list.Add(@{id=[string]$item.EntryID;subject=[string]$item.Subject;sender=[string]$item.SenderName;received=$item.ReceivedTime.ToString('yyyy-MM-dd HH:mm');store_id=[string]$folder.StoreID})
  };$result=@($list.ToArray())
 }
 else {
  $item=$session.GetItemFromID([string]$request.message_id,[string]$request.store_id)
  if($item.Class -ne 43){throw 'Not a mail item'}
  $list=[System.Collections.Generic.List[object]]::new()
  for($i=1;$i -le [Math]::Min($item.Attachments.Count,200);$i++){
   $attachment=$item.Attachments.Item($i);$name=[string]$attachment.FileName
   if($attachment.Type -ne 1 -or $name -notmatch '\.(xlsx|xlsm)$' -or $name -match '[\\/:*?"<>|\x00-\x1f]'){continue}
   $entry=@{id=[string]$i;name=$name;size=[long]$attachment.Size};$list.Add($entry)
   if($request.operation -eq 'download' -and [string]$request.attachment_id -eq [string]$i){
    if($name -ne [string]$request.expected_name -or [long]$attachment.Size -ne [long]$request.expected_size -or [long]$attachment.Size -gt 104857600){throw 'Attachment changed'}
    $attachment.SaveAsFile([string]$request.destination);$result=$entry;break
   }
  }
  if($request.operation -eq 'attachments'){$result=@($list.ToArray())}elseif(!$result){throw 'Attachment unavailable'}
 }
 @{type='result';data=$result}|ConvertTo-Json -Depth 8 -Compress|ForEach-Object{[Console]::Out.WriteLine($_)}
} catch {
 # Never return COM exception details: they can contain mailbox content or local paths.
 @{type='error';message='Outlook Classic could not complete this request. Check that Classic Outlook is installed, your profile is open, and the selected folder/message still exists. If Outlook asks for access, respond in Outlook.'}|ConvertTo-Json -Compress|ForEach-Object{[Console]::Out.WriteLine($_)}
 exit 1
}
