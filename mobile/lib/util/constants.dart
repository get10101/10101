// This file contains global constants.
// Note that global constants should only be use sparingly.
// They make sense for e.g. keys to identify widgets for testing.
// Hint: If you want to define a color it is recommended to use extend a Theme rather than adding a constant.

import 'package:flutter/material.dart';

// ############################################################################
// Common keys

const _tabs = "tabs/";
const _button = "button/";
const _slider = "slider/";

// ############################################################################
// Main keys

const _wallet = "wallet/";
const _trade = "trade/";

// path on screen

const _bottomSheet = "bottom_sheet/";
const _confirmSheet = "confirm/";

// concrete selectors

const _buy = "buy";
const _sell = "sell";
const _positions = "positions";
const _orders = "orders";

const tradeScreenTabsOrders = Key(_trade + _tabs + _orders);
const tradeScreenTabsPositions = Key(_trade + _tabs + _positions);

const tradeScreenButtonBuy = Key(_trade + _button + _buy);
const tradeScreenButtonSell = Key(_trade + _button + _sell);

const tradeScreenBottomSheetTabsBuy = Key(_trade + _bottomSheet + _tabs + _buy);
const tradeScreenBottomSheetTabsSell = Key(_trade + _bottomSheet + _tabs + _sell);

const tradeScreenBottomSheetButtonBuy = Key(_trade + _bottomSheet + _button + _buy);
const tradeScreenBottomSheetButtonSell = Key(_trade + _bottomSheet + _button + _sell);

const tradeScreenBottomSheetConfirmationSliderBuy =
    Key(_trade + _bottomSheet + _confirmSheet + _slider + _buy);
const tradeScreenBottomSheetConfirmationSliderSell =
    Key(_trade + _bottomSheet + _confirmSheet + _slider + _sell);

const tradeScreenBottomSheetConfirmationSliderButtonBuy =
    Key(_trade + _bottomSheet + _confirmSheet + _slider + _button + _buy);
const tradeScreenBottomSheetConfirmationSliderButtonSell =
    Key(_trade + _bottomSheet + _confirmSheet + _slider + _button + _sell);

const tabWallet = Key(_tabs + _wallet);
const tabTrade = Key(_tabs + _trade);
