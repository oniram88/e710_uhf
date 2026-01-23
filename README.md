### Connection through Ethernet
```shell
sudo ethtool -s enp8s0 speed 10 duplex half autoneg off
sudo ip link set enp8s0 down
sudo ip link set enp8s0 up
ip link show enp8s0

```
risultato:
```shell
2: enp8s0: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 qdisc fq_codel state UP mode DEFAULT group default qlen 1000
link/ether bc:fc:e7:1a:8d:36 brd ff:ff:ff:ff:ff:ff
```

deve essere state UP


#### Passaggio ARP
```shell
arp -n

Address                  HWtype  HWaddress           Flags Mask            Iface
172.20.0.2               ether   8a:bc:01:58:6e:03   C                     br-d93f7c90443a
192.168.178.1            ether   3c:37:12:41:15:68   C                     wlp9s0

ping -c 1 192.168.0.178

PING 192.168.0.178 (192.168.0.178) 56(84) bytes of data.
64 bytes from 192.168.0.178: icmp_seq=1 ttl=128 time=0.603 ms

--- 192.168.0.178 ping statistics ---
1 packets transmitted, 1 received, 0% packet loss, time 0ms
rtt min/avg/max/mdev = 0.603/0.603/0.603/0.000 ms

arp -n
Address                  HWtype  HWaddress           Flags Mask            Iface
172.20.0.2               ether   8a:bc:01:58:6e:03   C                     br-d93f7c90443a
192.168.178.1            ether   3c:37:12:41:15:68   C                     wlp9s0
192.168.0.178            ether   50:54:7b:b4:1f:51   C                     enp8s0
```

