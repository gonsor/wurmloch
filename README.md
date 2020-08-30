# wurmloch

wurmloch turns a folder on your filesystem into a wormhole. Everything you drop on it gets sorted according to your own rules. How do these rules look? Like this:

```yaml
- pattern: "*.jpg"
  target: "/home/foo/pictures"
- pattern: "*.pdf"
  target: "/home/foo/documents"

  ...
```

Drop your jpgs on it, they land in your picture folder. Drop your pdfs, they land in your document folder. And so on. Patterns as Globs, targets as folders. You get the idea.

Works on Linux, Windows and Mac OS.

## Usage
 
First, you create a new folder on your disk. Name it anything. You can turn this folder into a wormhole like this:

_Linux/Mac_

`wurmloch /path/to/wormhole/folder >> /var/log/wurmloch.log`

_Windows_

`wurmloch.exe C:\Path\To\Wormhole\Folder >> C:\Users\Foo\Wurmloch.log`

It is a good idea to put this into your autostart as your wormhole will always be active then.

## Configuration

After the first startup, a rule configuration file will be created for you. The location depends on your operating system.

_Linux_

`/home/foo/.config/Wurmloch/rules.yaml`

_Windows_

`C:\Users\Foo\AppData\Roaming\Wurmloch\rules.yaml`

_Mac_

`/Users/Foo/Library/Application Support/Wurmloch/rules.yaml`

Open the rule file with any text editor. Some example rules are provided. Add all the rules you need.

- If multiple rules match for something that is dropped into the wormhole, the rule that is higher up takes precedence.
- If you save while the wurmloch program is already running, the file gets automatically reparsed.
- If you made errors, they will appear in the logfile.

## Troubeshooting

If a rule is not not considered, some behaviour is unexpected or the universe is crumbling, you can get more information by increasing the log level:

_Linux/Mac_

`WURMLOCH_LOG=debug`

_Windows_

`SET WURMLOCH_LOG=debug`

Restart wurmloch afterwards, drop the file again and check the log.