import 'package:flutter/material.dart';

final GlobalKey<NavigatorState> rootNavigatorKey = GlobalKey<NavigatorState>(debugLabel: 'root');
final GlobalKey<NavigatorState> shellNavigatorKey = GlobalKey<NavigatorState>(debugLabel: 'shell');
final GlobalKey<NavigatorState> shellNavigatorKeyWallet =
    GlobalKey<NavigatorState>(debugLabel: 'wallet');
final GlobalKey<NavigatorState> shellNavigatorKeyTrading =
    GlobalKey<NavigatorState>(debugLabel: 'trading');
final GlobalKey<NavigatorState> shellNavigatorKeySettings =
    GlobalKey<NavigatorState>(debugLabel: 'settings');
