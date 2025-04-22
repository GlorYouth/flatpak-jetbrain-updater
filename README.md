# flatpak-jetbrain-updater
A Rust Tool to Update Flatpak Jetbrains' IDEs Automatically.

## 简单介绍

目前未经严格测试，只是个半成品，本工具主要用于 flatpak 的 JetBrains IDEs 更新，目前仅在 WebStorm 测试过， 使用此工具，你可以即时更新 Flatpak JetBrains IDE，而不用被官方 Flatpak 慢如蜗牛的更新速度恶心。  

需要注意的是，本工具仅更新IDE部分，对其他 flatpak-builder 需要的工具包甚至桌面环境相关包更新并未涉及，故仍然在一定程度上要与官方 repo 同步  

本 repo 下，有 webstorm build 关键xml修改前(_bak文件)和修改后的文件样例，以作参考  

至于使用本软件，你只需要构建，然后丢到 Clone了 JetBrain repo 的文件夹下，运行即可。  

目前限制以下 JetBrains IDEs 使用:  

CLion,  
RustRover,  
WebStorm,  
GoLand,  
Pycharm-Community,

若你有rust基础或者使用AI，可以在 /src/resolve/product.rs 内添加其他IDE进行测试  

当然，欢迎各位 Fork and Pull  