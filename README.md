# yagna-named-extension
Extension adds node name to yagna commands, which return table with `nodeId` column.

## Example

Running built-in yagna command:

`yagna net sessions`
![Screenshot from 2022-05-27 11-08-12](https://user-images.githubusercontent.com/10420306/170669482-9899c580-f680-411c-a28c-ce8bf025c710.png)

running with extension:

`yagna named net sessions`
![Screenshot from 2022-05-27 11-08-37](https://user-images.githubusercontent.com/10420306/170669488-d1142342-2f12-4b03-aaac-8ee8bf93d00d.png)


## How it works?

`yagna-named` subscribes `Demand` on market to get Offers from Providers. `Offer` contains `nodeID` field, which will be used by extension as Node name.
Node names are cached on disk and later used when running commands, so we don't query market each time, we execute a command.


## Using extension

### Collecting node names

First we need to collect node names and save them in cache.
We can do this manually by calling command:

`yagna named collect`

or we can register extension to autostart:

`yagna extension register named colllect`

Now extension will be started together with `yagna` daemon and will update node names cache in real time.

### Running yagna commands

You can use it with any command that displays table with `nodeId` column, by prefixing commands with `named`


