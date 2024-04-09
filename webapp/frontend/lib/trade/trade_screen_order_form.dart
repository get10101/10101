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
  const NewOrderForm({super.key});

  @override
  State<NewOrderForm> createState() => _NewOrderForm();
}

class _NewOrderForm extends State<NewOrderForm> {
  BestQuote? _quote;
  Usd? _quantity = Usd(100);
  Leverage _leverage = Leverage(1);
  DlcChannel? _openChannel;

  @override
  void initState() {
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    TenTenOneTheme theme = Theme.of(context).extension<TenTenOneTheme>()!;

    _quote = context.watch<QuoteChangeNotifier>().getBestQuote();
    _openChannel = context.watch<ChannelChangeNotifier>().getOpenChannel();

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
            }),
          ),
        ),
        spaceBetweenRows,
        Align(
          alignment: AlignmentDirectional.centerEnd,
          child: LeverageSlider(
            onLeverageChanged: (leverage) => setState(() {
              _leverage = Leverage(leverage);
            }),
            initialValue: _leverage.asDouble,
          ),
        ),
        spaceBetweenRows,
        spaceBetweenRows,
        Row(
          children: [
            Flexible(
              child: ElevatedButton(
                  onPressed: _quote != null
                      ? () {
                          Amount? fee = calculateFee(_quantity, _quote, true);
                          showDialog(
                              context: context,
                              builder: (BuildContext context) {
                                if (_openChannel != null) {
                                  return CreateOrderConfirmationDialog(
                                    direction: Direction.long,
                                    onConfirmation: () {},
                                    onCancel: () {},
                                    bestQuote: _quote!,
                                    fee: fee,
                                    leverage: _leverage,
                                    quantity: _quantity ?? Usd.zero(),
                                  );
                                } else {
                                  return CreateChannelConfirmationDialog(
                                      direction: Direction.long,
                                      onConfirmation: () {},
                                      onCancel: () {},
                                      bestQuote: _quote!,
                                      fee: fee,
                                      leverage: _leverage,
                                      quantity: _quantity ?? Usd.zero(),
                                      margin: (_quantity != null && _quote != null)
                                          ? calculateMargin(_quantity!, _quote!, _leverage, true)
                                          : Amount.zero());
                                }
                              });
                        }
                      : null,
                  style: ElevatedButton.styleFrom(
                      backgroundColor: theme.buy, minimumSize: const Size.fromHeight(50)),
                  child: const Text("Buy")),
            ),
            const SizedBox(width: 5),
            Flexible(
              child: ElevatedButton(
                  onPressed: _quote != null
                      ? () {
                          Amount? fee = calculateFee(_quantity, _quote, false);
                          showDialog(
                              context: context,
                              builder: (BuildContext context) {
                                if (_openChannel != null) {
                                  return CreateOrderConfirmationDialog(
                                    direction: Direction.short,
                                    onConfirmation: () {},
                                    onCancel: () {},
                                    bestQuote: _quote!,
                                    fee: fee,
                                    leverage: _leverage,
                                    quantity: _quantity ?? Usd.zero(),
                                  );
                                } else {
                                  return CreateChannelConfirmationDialog(
                                      direction: Direction.short,
                                      onConfirmation: () {},
                                      onCancel: () {},
                                      bestQuote: _quote!,
                                      fee: fee,
                                      leverage: _leverage,
                                      quantity: _quantity ?? Usd.zero(),
                                      margin: (_quantity != null)
                                          ? calculateMargin(_quantity!, _quote!, _leverage, false)
                                          : Amount.zero());
                                }
                              });
                        }
                      : null,
                  style: ElevatedButton.styleFrom(
                      backgroundColor: theme.sell, minimumSize: const Size.fromHeight(50)),
                  child: const Text("Sell")),
            ),
          ],
        ),
      ],
    );
  }
}
