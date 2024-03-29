import 'package:bitcoin_icons/bitcoin_icons.dart';
import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:get_10101/change_notifier/channel_change_notifier.dart';
import 'package:get_10101/change_notifier/quote_change_notifier.dart';
import 'package:get_10101/common/amount_text_input_form_field.dart';
import 'package:get_10101/common/calculations.dart';
import 'package:get_10101/common/direction.dart';
import 'package:get_10101/common/model.dart';
import 'package:get_10101/common/theme.dart';
import 'package:get_10101/settings/dlc_channel.dart';
import 'package:get_10101/trade/create_channel_confirmation_dialog.dart';
import 'package:get_10101/trade/create_order_confirmation_dialog.dart';
import 'package:get_10101/trade/leverage_slider.dart';
import 'package:get_10101/services/quote_service.dart';
import 'package:provider/provider.dart';

class NewOrderForm extends StatefulWidget {
  final bool isLong;

  // for now just to avoid division by 0 errors, later we should introduce a maintenance margin
  final double maintenanceMargin = 0.001;

  const NewOrderForm({super.key, required this.isLong});

  @override
  State<NewOrderForm> createState() => _NewOrderForm();
}

class _NewOrderForm extends State<NewOrderForm> {
  BestQuote? _quote;
  Usd? _quantity = Usd(100);
  Leverage _leverage = Leverage(1);
  bool isBuy = true;
  DlcChannel? _openChannel;

  final TextEditingController _marginController = TextEditingController();
  final TextEditingController _liquidationPriceController = TextEditingController();
  final TextEditingController _latestPriceController = TextEditingController();

  @override
  void initState() {
    super.initState();
    isBuy = widget.isLong;
  }

  @override
  Widget build(BuildContext context) {
    TenTenOneTheme theme = Theme.of(context).extension<TenTenOneTheme>()!;
    Color buyButtonColor = isBuy ? theme.buy : theme.inactiveButtonColor;
    Color sellButtonColor = isBuy ? theme.inactiveButtonColor : theme.sell;
    final direction = isBuy ? Direction.long : Direction.short;

    _quote = context.watch<QuoteChangeNotifier>().getBestQuote();
    _openChannel = context.watch<ChannelChangeNotifier>().getOpenChannel();

    updateOrderValues();

    const spaceBetweenRows = SizedBox(height: 10);
    return Column(
      children: [
        const SizedBox(height: 20),
        Align(
          alignment: AlignmentDirectional.centerEnd,
          child: AmountInputField(
            initialValue: _quantity,
            enabled: true,
            label: "Quantity",
            textAlign: TextAlign.right,
            suffixIcon: const Icon(FontAwesomeIcons.dollarSign),
            onChanged: (quantity) => setState(() {
              _quantity = Usd.parseString(quantity);
              updateOrderValues();
            }),
          ),
        ),
        spaceBetweenRows,
        Align(
          alignment: AlignmentDirectional.centerEnd,
          child: LeverageSlider(
            onLeverageChanged: (leverage) => setState(() {
              _leverage = Leverage(leverage);
              updateOrderValues();
            }),
            initialValue: _leverage.asDouble,
          ),
        ),
        spaceBetweenRows,
        Align(
          alignment: AlignmentDirectional.centerEnd,
          child: AmountInputField(
            enabled: false,
            label: isBuy ? "Ask Price" : "Bid Price",
            textAlign: TextAlign.right,
            suffixIcon: const Icon(FontAwesomeIcons.dollarSign),
            controller: _latestPriceController,
          ),
        ),
        spaceBetweenRows,
        Align(
          alignment: AlignmentDirectional.centerEnd,
          child: AmountInputField(
            enabled: false,
            label: "Margin",
            suffixIcon: const Icon(BitcoinIcons.satoshi_v1),
            textAlign: TextAlign.right,
            controller: _marginController,
          ),
        ),
        spaceBetweenRows,
        Align(
          alignment: AlignmentDirectional.centerEnd,
          child: AmountInputField(
            enabled: false,
            label: "Liquidation",
            suffixIcon: const Icon(FontAwesomeIcons.dollarSign),
            textAlign: TextAlign.right,
            controller: _liquidationPriceController,
          ),
        ),
        spaceBetweenRows,
        spaceBetweenRows,
        Align(
          alignment: AlignmentDirectional.center,
          child: ElevatedButton(
              onPressed: () {
                Amount? fee = calculateFee(_quantity, _quote, isBuy);
                showDialog(
                    context: context,
                    builder: (BuildContext context) {
                      if (_openChannel != null) {
                        return CreateOrderConfirmationDialog(
                          direction: direction,
                          onConfirmation: () {},
                          onCancel: () {},
                          bestQuote: _quote,
                          fee: fee,
                          leverage: _leverage,
                          quantity: _quantity ?? Usd.zero(),
                        );
                      } else {
                        return CreateChannelConfirmationDialog(
                            direction: direction,
                            onConfirmation: () {},
                            onCancel: () {},
                            bestQuote: _quote == null
                                ? BestQuote(ask: Price.zero(), bid: Price.zero(), fee: 0.0)
                                : _quote!,
                            fee: fee,
                            leverage: _leverage,
                            quantity: _quantity ?? Usd.zero(),
                            margin: (_quantity != null && _quote != null)
                                ? calculateMargin(_quantity!, _quote!, _leverage, isBuy)
                                : Amount.zero());
                      }
                    });
              },
              style: ElevatedButton.styleFrom(
                  backgroundColor: isBuy ? buyButtonColor : sellButtonColor,
                  minimumSize: const Size.fromHeight(50)),
              child: (isBuy ? const Text("Buy") : const Text("Sell"))),
        ),
      ],
    );
  }

  void updateOrderValues() {
    if (_quantity != null && _quote != null) {
      _marginController.text = calculateMargin(_quantity!, _quote!, _leverage, isBuy).formatted();
      _liquidationPriceController.text =
          calculateLiquidationPrice(_quantity!, _quote!, _leverage, widget.maintenanceMargin, isBuy)
              .formatted();
    }

    if (isBuy && _quote != null && _quote!.ask != null) {
      _latestPriceController.text = _quote!.ask!.formatted();
    } else if (!isBuy && _quote != null && _quote!.bid != null) {
      _latestPriceController.text = _quote!.bid!.formatted();
    }
  }
}
