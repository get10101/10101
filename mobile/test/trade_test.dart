import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:get_10101/common/amount_denomination_change_notifier.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/order_change_notifier.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:get_10101/features/trade/trade_screen.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:get_10101/features/trade/trade_value_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/util/constants.dart';
import 'package:go_router/go_router.dart';
import 'package:mockito/annotations.dart';
import 'package:mockito/mockito.dart';
import 'package:provider/provider.dart';

@GenerateNiceMocks([MockSpec<TradeValuesService>()])
import 'package:get_10101/features/trade/application/trade_values_service.dart';

@GenerateNiceMocks([MockSpec<OrderService>()])
import 'package:get_10101/features/trade/application/order_service.dart';

@GenerateNiceMocks([MockSpec<PositionService>()])
import 'package:get_10101/features/trade/application/position_service.dart';

@GenerateNiceMocks([MockSpec<WalletService>()])
import 'package:get_10101/features/wallet/application/wallet_service.dart';

import 'trade_test.mocks.dart';

final GoRouter _router = GoRouter(
  initialLocation: TradeScreen.route,
  routes: [
    GoRoute(
        path: TradeScreen.route,
        builder: (BuildContext context, GoRouterState state) {
          return const TradeScreen();
        }),
  ],
);

class TestWrapperWithTradeTheme extends StatelessWidget {
  final Widget child;

  const TestWrapperWithTradeTheme({super.key, required this.child});

  @override
  Widget build(BuildContext context) {
    return MaterialApp.router(
      // TODO: We could consider using the Navigator instead of GoRouter to close the bottom sheet again
      // Need GoRouter otherwise closing the bottom sheet after confirmation fails
      routerConfig: _router,
      theme: ThemeData(
        primarySwatch: Colors.blue,
        extensions: const <ThemeExtension<dynamic>>[
          // Need the trade theme otherwise the trade widgets that rely on it can't find it on the context and fail to render
          TradeTheme(),
        ],
      ),
    );
  }
}

void main() {
  testWidgets('Given trade screen when completing buy flow then market order is submitted',
      (tester) async {
    MockOrderService orderService = MockOrderService();
    MockPositionService positionService = MockPositionService();
    MockTradeValuesService tradeValueService = MockTradeValuesService();
    MockWalletService walletService = MockWalletService();

    // TODO: we could make this more resilient in the underlying components...
    // return dummies otherwise the fields won't be initialized correctly
    when(tradeValueService.calculateMargin(
            price: anyNamed('price'),
            quantity: anyNamed('quantity'),
            leverage: anyNamed('leverage')))
        .thenReturn(Amount(2000));
    when(tradeValueService.calculateLiquidationPrice(
            price: anyNamed('price'),
            leverage: anyNamed('leverage'),
            direction: anyNamed('direction')))
        .thenReturn(10000);
    when(tradeValueService.calculateQuantity(
            price: anyNamed('price'), leverage: anyNamed('leverage'), margin: anyNamed('margin')))
        .thenReturn(100);

    SubmitOrderChangeNotifier submitOrderChangeNotifier = SubmitOrderChangeNotifier(orderService);

    await tester.pumpWidget(MultiProvider(providers: [
      ChangeNotifierProvider(create: (context) => TradeValuesChangeNotifier(tradeValueService)),
      ChangeNotifierProvider(create: (context) => submitOrderChangeNotifier),
      ChangeNotifierProvider(create: (context) => OrderChangeNotifier(orderService)),
      ChangeNotifierProvider(
          create: (context) => PositionChangeNotifier(positionService, orderService)),
      ChangeNotifierProvider(create: (context) => AmountDenominationChangeNotifier()),
      ChangeNotifierProvider(create: (context) => WalletChangeNotifier(walletService)),
    ], child: const TestWrapperWithTradeTheme(child: TradeScreen())));

    await tester.pumpAndSettle();

    expect(find.byKey(tradeScreenButtonBuy), findsOneWidget);

    // Open bottom sheet
    await tester.tap(find.byKey(tradeScreenButtonBuy));
    await tester.pumpAndSettle();

    expect(find.byKey(tradeScreenBottomSheetButtonBuy), findsOneWidget);

    // click buy button in bottom sheet
    await tester.tap(find.byKey(tradeScreenBottomSheetButtonBuy));
    await tester.pumpAndSettle();

    expect(find.byKey(tradeScreenBottomSheetConfirmationSliderButtonBuy), findsOneWidget);

    // TODO: This is not optimal because if we re-style the component this test will likely break.
    // Drag to confirm
    await tester.timedDrag(find.byKey(tradeScreenBottomSheetConfirmationSliderButtonBuy),
        const Offset(275, 0), const Duration(seconds: 2),
        pointer: 7);

    verify(orderService.submitMarketOrder(any, any, any, any)).called(1);
  });
}
