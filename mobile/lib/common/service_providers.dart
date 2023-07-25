import 'package:get_10101/features/trade/application/trade_values_service.dart';
import 'package:get_10101/features/trade/candlestick_change_notifier.dart';
import 'package:get_10101/features/trade/order_change_notifier.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:get_10101/features/trade/trade_value_change_notifier.dart';
import 'package:get_10101/features/wallet/send_payment_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/util/environment.dart';
import 'package:get_10101/common/amount_denomination_change_notifier.dart';
import 'package:provider/provider.dart';
import 'package:provider/single_child_widget.dart';
import 'package:get_10101/features/trade/application/candlestick_service.dart';
import 'package:get_10101/features/trade/application/order_service.dart';
import 'package:get_10101/features/trade/application/position_service.dart';
import 'package:get_10101/features/wallet/application/wallet_service.dart';
import 'package:get_10101/common/application/channel_info_service.dart';
import 'package:get_10101/common/domain/init_service.dart';

List<SingleChildWidget> createServiceProviders(
    TradeValuesService tradeValues,
    ChannelInfoService channelInfo,
    OrderService order,
    PositionService position,
    WalletService wallet,
    CandlestickService candleStick,
    InitService init) {
  return [
    ChangeNotifierProvider(
        create: (context) =>
            TradeValuesChangeNotifier(tradeValues, channelInfo)),
    ChangeNotifierProvider(
        create: (context) => AmountDenominationChangeNotifier()),
    ChangeNotifierProvider(
        create: (context) => SubmitOrderChangeNotifier(order)),
    ChangeNotifierProvider(create: (context) => OrderChangeNotifier(order)),
    ChangeNotifierProvider(
        create: (context) => PositionChangeNotifier(position)),
    ChangeNotifierProvider(create: (context) => WalletChangeNotifier(wallet)),
    ChangeNotifierProvider(
        create: (context) => SendPaymentChangeNotifier(wallet)),
    ChangeNotifierProvider(
        create: (context) => CandlestickChangeNotifier(candleStick)),
    Provider(create: (context) => Environment.parse()),
    Provider(create: (context) => channelInfo),
    Provider(create: (context) => init),
  ];
}
